// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `workflow prod-verify`: production verification of a STABLE release.
//!
//! It runs immediately after the (manual) Upload PROD on a stable tag, and it
//! is the ONE home for 生产发布验证 — a consumer repo must not trigger release
//! pipelines itself (devine_groups rule, 2026-09-28).
//!
//! Order matters: the prod artifact must be live (proving the owner really
//! published the stable), then each app's `main` pipeline is triggered and
//! its `/version` polled. integration-02, the clean-install check, goes first.

use crate::commands::pipeline_run;

use super::flags::{parse_with, scan_int, Take, WorkflowFlags};
use super::gate::workflow_want_version;
use super::http::{
    poll_app_version, poll_artifactory, APP_VERSION_POLL_STEP, ARTIFACTORY_POLL_STEP,
};
use super::model::{TestApp, WorkflowConfig};
use super::prod_apps::{is_stable_version, select_apps};
use super::registry::Context;
use super::util::{artifactory_url, go_slice, non_empty_or};
use std::time::Duration;

/// The flags of `workflow prod-verify`.
#[derive(Debug, Clone, Default)]
pub struct ProdVerifyFlags {
    common: WorkflowFlags,
    version: String,
    from: String,
    apps: String,
    artifact_timeout: i64,
    wait_timeout: i64,
    poll_timeout: i64,
    skip_artifact: bool,
}

fn parse_prod_verify(args: &[String]) -> (ProdVerifyFlags, Vec<String>) {
    let mut flags = ProdVerifyFlags::default();
    let rest = parse_with(args, |arg, next| {
        if let Some(take) = flags.common.take_common(arg, next) {
            return Some(take);
        }
        match (arg, next) {
            ("--version", Some(value)) => {
                flags.version = value.to_string();
                Some(Take::Value)
            }
            ("--from", Some(value)) => {
                flags.from = value.to_string();
                Some(Take::Value)
            }
            ("--apps", Some(value)) => {
                flags.apps = value.to_string();
                Some(Take::Value)
            }
            ("--artifact-timeout", Some(value)) => {
                flags.artifact_timeout = scan_int(value);
                Some(Take::Value)
            }
            ("--wait-timeout", Some(value)) => {
                flags.wait_timeout = scan_int(value);
                Some(Take::Value)
            }
            ("--poll-timeout", Some(value)) => {
                flags.poll_timeout = scan_int(value);
                Some(Take::Value)
            }
            ("--skip-artifact", _) => {
                flags.skip_artifact = true;
                Some(Take::Rest)
            }
            _ => None,
        }
    });
    (flags, rest)
}

/// `workflow prod-verify`.
pub fn workflow_prod_verify(
    args: &[String],
    cfg: &WorkflowConfig,
    ctx: &Context,
) -> Result<i32, String> {
    let (flags, positional) = parse_prod_verify(args);
    if positional.len() > 1 {
        return Err(format!(
            "prod-verify: at most one positional <version> (got {})",
            go_slice(&positional)
        ));
    }
    let version = resolve_version(&flags, &positional, cfg, ctx)?;
    if !is_stable_version(&version) {
        return Err(format!(
            "prod-verify: REFUSED — {version:?} is not a stable release (x.y.z). \
             Production verification is stable-only; rc candidates are covered by the rc chain \
             (workflow release-rc / test-rc)"
        ));
    }
    let apps = select_apps(cfg, &flags.apps);
    if apps.is_empty() {
        return Err(format!(
            "prod-verify: no test apps selected (--apps {:?} matched nothing in the catalog)",
            flags.apps
        ));
    }
    println!(
        "[prod-verify] version={version} (stable) | {} app(s)",
        apps.len()
    );
    for app in &apps {
        println!(
            "  - {:<38} {:<42} → {}",
            app.name, app.project_path, app.version_url
        );
    }
    if flags.common.dry_run {
        println!(
            "[prod-verify] dry-run: would verify prod artifactory for {version}, \
             then trigger + /version-poll {} app pipeline(s) on main",
            apps.len()
        );
        return Ok(0);
    }
    check_artifact(cfg, &version, &flags, ARTIFACTORY_POLL_STEP)?;

    let wait_timeout = if flags.wait_timeout <= 0 {
        900
    } else {
        flags.wait_timeout
    };
    let poll_timeout = if flags.poll_timeout <= 0 {
        600
    } else {
        flags.poll_timeout
    };
    let mut failures: Vec<String> = Vec::new();
    for app in &apps {
        println!("[prod-verify] === {} ({}) ===", app.name, app.project_path);
        if let Err(error) = verify_app(
            app,
            &version,
            wait_timeout,
            poll_timeout,
            flags.common.verbose,
        ) {
            println!("  FAIL — {error}");
            failures.push(format!("{}: {error}", app.name));
        }
    }
    println!("[prod-verify] ============================================");
    if failures.is_empty() {
        println!(
            "[prod-verify] PASS — all {} app(s) verified on {version}",
            apps.len()
        );
        return Ok(0);
    }
    Err(format!(
        "prod-verify: FAIL — {} app(s) failed for {version}: {}",
        failures.len(),
        failures.join(" | ")
    ))
}

/// The version to verify: `--version`, `--from`, the positional, the VERSION
/// file, in that order.
fn resolve_version(
    flags: &ProdVerifyFlags,
    positional: &[String],
    cfg: &WorkflowConfig,
    ctx: &Context,
) -> Result<String, String> {
    let mut version = non_empty_or(&flags.version, &flags.from).trim().to_string();
    if version.is_empty() {
        if let Some(first) = positional.first() {
            version = first.trim().to_string();
        }
    }
    if version.is_empty() {
        version = workflow_want_version(cfg, ctx);
    }
    if version.is_empty() {
        return Err(
            "prod-verify: cannot determine the stable version — pass <x.y.z> or \
                    --from <x.y.z> (or run from inside the binary repo where VERSION is present)"
                .to_string(),
        );
    }
    Ok(version)
}

/// Waits for the PROD artifact, unless `--skip-artifact`.
fn check_artifact(
    cfg: &WorkflowConfig,
    version: &str,
    flags: &ProdVerifyFlags,
    step: Duration,
) -> Result<(), String> {
    if flags.skip_artifact {
        return Ok(());
    }
    let timeout = if flags.artifact_timeout <= 0 {
        600
    } else {
        flags.artifact_timeout
    };
    let url = artifactory_url(cfg, &cfg.artifactory.prod_path, version);
    println!("[prod-verify] waiting for prod artifactory artifact: {url}");
    let info = poll_artifactory(
        "prod-verify",
        &url,
        cfg.artifactory.min_size_b,
        timeout,
        flags.common.verbose,
        step,
    )
    .map_err(|error| {
        format!(
            "prod-verify: {error} — did the owner click ▶ Upload PROD? (it is manual on stable tags)"
        )
    })?;
    println!("  OK — artifact present ({} bytes)", info.len);
    Ok(())
}

/// Triggers one app's pipeline and asserts its `/version` serves `version`.
fn verify_app(
    app: &TestApp,
    version: &str,
    wait_timeout: i64,
    poll_timeout: i64,
    verbose: bool,
) -> Result<(), String> {
    let pr_args: Vec<String> = vec![
        app.project_path.clone(),
        "--ref".to_string(),
        "main".to_string(),
        "--output".to_string(),
        "text".to_string(),
        "--wait".to_string(),
        "--wait-timeout".to_string(),
        wait_timeout.to_string(),
        "--expect-success".to_string(),
    ];
    if verbose {
        println!(
            "  triggering {} pipeline on main (wait ≤ {wait_timeout}s)",
            app.project_path
        );
    }
    pipeline_run::pipeline_run(&pr_args)?;
    println!(
        "  polling {} for app_version == {version} (timeout {poll_timeout}s)",
        app.version_url
    );
    let got = poll_app_version(
        &app.version_url,
        version,
        poll_timeout,
        verbose,
        APP_VERSION_POLL_STEP,
    )?;
    println!("  OK — {} serves {got}", app.name);
    Ok(())
}

#[cfg(test)]
mod tests;
