// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `workflow list`, `workflow apps`, `workflow test-rc` and
//! `workflow integration-rc`.
//!
//! The two rc commands reuse the `pipeline-run` machinery for the GitLab
//! trigger and the optional wait, then add the authoritative `/version` poll
//! on top. The FAAS CI resolves the newest UAT/PROD binary by itself, so no
//! version variable is ever passed.

use serde::Serialize;

use super::flags::{parse_common, parse_with, scan_int, Take, WorkflowFlags};
use super::gate::{trigger_and_poll_app, workflow_want_version};
use super::model::{WorkflowConfig, DEFAULT_WORKFLOW_NAME};
use super::registry::{Context, Registry};
use super::util::{go_slice, non_empty_or, write_output_json};

/// One row of `workflow list`.
#[derive(Debug, Clone, Serialize)]
struct WorkflowListInfo {
    name: String,
    binary_repo: String,
    project: String,
    builtin: bool,
    test_apps: Vec<String>,
    roles: Vec<String>,
}

/// `workflow list`: every named workflow, with its binary repo and apps.
pub fn workflow_list(args: &[String], reg: &Registry) -> Result<i32, String> {
    let (flags, positional) = parse_common(args)?;
    if !positional.is_empty() {
        return Err(format!(
            "list: unexpected positional args {}",
            go_slice(&positional)
        ));
    }
    let rows = list_rows(reg);
    if flags.output == "json" {
        write_output_json(&rows)?;
        return Ok(0);
    }
    println!(
        "{:<30} {:<8}  {:<28}  test apps",
        "workflow", "builtin", "binary repo"
    );
    for row in &rows {
        let builtin = if row.builtin { "yes" } else { "no" };
        let mut apps = row.test_apps.join(", ");
        if apps.is_empty() {
            apps = "(none)".to_string();
        }
        println!(
            "{:<30} {:<8}  {:<28}  {}",
            row.name, builtin, row.binary_repo, apps
        );
    }
    println!(
        "\n{} workflow(s) known. Default (no name given): {DEFAULT_WORKFLOW_NAME}",
        rows.len()
    );
    Ok(0)
}

/// The rows `workflow list` prints, one per known workflow.
fn list_rows(reg: &Registry) -> Vec<WorkflowListInfo> {
    reg.iter()
        .map(|(name, cfg)| WorkflowListInfo {
            name: name.clone(),
            binary_repo: cfg.binary_repo.name.clone(),
            project: cfg.binary_repo.project_path.clone(),
            builtin: name == DEFAULT_WORKFLOW_NAME,
            test_apps: cfg.test_apps.iter().map(|app| app.name.clone()).collect(),
            roles: cfg.test_apps.iter().map(|app| app.role.clone()).collect(),
        })
        .collect()
}

/// `workflow apps`: the test FAAS app catalog.
pub fn workflow_apps(args: &[String], cfg: &WorkflowConfig) -> Result<i32, String> {
    let (flags, positional) = parse_common(args)?;
    if !positional.is_empty() {
        return Err(format!(
            "apps: unexpected positional args {}",
            go_slice(&positional)
        ));
    }
    if flags.output == "json" {
        write_output_json(&cfg.test_apps)?;
        return Ok(0);
    }
    println!(
        "workflow {} — binary runtime: {} ({})",
        non_empty_or(&cfg.name, DEFAULT_WORKFLOW_NAME),
        cfg.binary_repo.name,
        cfg.binary_repo.project_path
    );
    println!("Artifactory UAT/PROD: {}", cfg.artifactory.base_url);
    println!();
    println!(
        "{:<40} {:<14} {:<5} {:<8}  version endpoint",
        "app", "role", "debug", "polluted"
    );
    for app in &cfg.test_apps {
        let debug = if app.debug { "on" } else { "off" };
        let polluted = if app.polluted { "yes" } else { "no" };
        println!(
            "{:<40} {:<14} {:<5} {:<8}  {}",
            app.name, app.role, debug, polluted, app.version_url
        );
    }
    println!();
    println!("Roles:");
    println!("  dev-test     — smoke-test for new rc builds. /version == rc is the AUTHORITATIVE release signal.");
    println!("  integration  — production-like FAAS exercise. 01 has debug on + possibly polluted QConfig;");
    println!("                 02 is clean + debug off (closest to a clean-install check).");
    Ok(0)
}

/// The flags of `workflow test-rc`.
#[derive(Debug, Clone, Default)]
pub struct TestRcFlags {
    common: WorkflowFlags,
    wait: bool,
    wait_timeout: i64,
    poll_timeout: i64,
    poll_version: bool,
}

fn parse_test_rc(args: &[String]) -> (TestRcFlags, Vec<String>) {
    let mut flags = TestRcFlags::default();
    let rest = parse_with(args, |arg, next| {
        if let Some(take) = flags.common.take_common(arg, next) {
            return Some(take);
        }
        match (arg, next) {
            ("--wait", _) => {
                flags.wait = true;
                Some(Take::Rest)
            }
            ("--wait-timeout", Some(value)) => {
                flags.wait_timeout = scan_int(value);
                Some(Take::Value)
            }
            ("--poll-timeout", Some(value)) => {
                flags.poll_timeout = scan_int(value);
                Some(Take::Value)
            }
            ("--poll-version", _) => {
                flags.poll_version = true;
                Some(Take::Rest)
            }
            _ => None,
        }
    });
    (flags, rest)
}

/// `workflow test-rc`: trigger the dev-test pipeline and poll `/version`.
pub fn workflow_test_rc(
    args: &[String],
    cfg: &WorkflowConfig,
    ctx: &Context,
) -> Result<i32, String> {
    let (flags, positional) = parse_test_rc(args);
    if !positional.is_empty() {
        return Err(format!(
            "test-rc: unexpected positional args {}",
            go_slice(&positional)
        ));
    }
    let app = cfg.find_test_app("")?;
    let want = workflow_want_version(cfg, ctx);
    println!(
        "[workflow test-rc] app={} project={} want_app_version={want:?}",
        app.name, app.project_path
    );
    if flags.common.dry_run {
        println!(
            "[workflow test-rc] dry-run: would trigger {} pipeline on main (no vars), then poll {} for {want}",
            app.project_path, app.version_url
        );
        return Ok(0);
    }
    trigger_and_poll_app(
        app,
        &want,
        flags.wait,
        flags.wait_timeout,
        flags.poll_version,
        flags.poll_timeout,
        flags.common.verbose,
    )
    .map_err(|error| format!("test-rc: {error}"))?;
    Ok(0)
}

/// The flags of `workflow integration-rc`.
#[derive(Debug, Clone, Default)]
pub struct IntegrationRcFlags {
    common: WorkflowFlags,
    app: String,
    wait: bool,
    wait_timeout: i64,
    poll_timeout: i64,
    poll_version: bool,
}

fn parse_integration_rc(args: &[String]) -> (IntegrationRcFlags, Vec<String>) {
    let mut flags = IntegrationRcFlags::default();
    let rest = parse_with(args, |arg, next| {
        if let Some(take) = flags.common.take_common(arg, next) {
            return Some(take);
        }
        match (arg, next) {
            ("--app", Some(value)) => {
                flags.app = value.to_string();
                Some(Take::Value)
            }
            ("--wait", _) => {
                flags.wait = true;
                Some(Take::Rest)
            }
            ("--wait-timeout", Some(value)) => {
                flags.wait_timeout = scan_int(value);
                Some(Take::Value)
            }
            ("--poll-timeout", Some(value)) => {
                flags.poll_timeout = scan_int(value);
                Some(Take::Value)
            }
            ("--poll-version", _) => {
                flags.poll_version = true;
                Some(Take::Rest)
            }
            _ => None,
        }
    });
    (flags, rest)
}

/// `workflow integration-rc`: trigger one integration app, optionally verify.
pub fn workflow_integration_rc(
    args: &[String],
    cfg: &WorkflowConfig,
    ctx: &Context,
) -> Result<i32, String> {
    let (flags, positional) = parse_integration_rc(args);
    if !positional.is_empty() {
        return Err(format!(
            "integration-rc: unexpected positional args {}",
            go_slice(&positional)
        ));
    }
    let app = cfg.find_test_app(&flags.app)?;
    let want = workflow_want_version(cfg, ctx);
    println!(
        "[workflow integration-rc] app={} project={} role={} debug={} polluted={} want={want:?}",
        app.name, app.project_path, app.role, app.debug, app.polluted
    );
    if flags.common.dry_run {
        println!(
            "[workflow integration-rc] dry-run: would trigger {} pipeline on main (no vars), then poll {} for {want}",
            app.project_path, app.version_url
        );
        return Ok(0);
    }
    trigger_and_poll_app(
        app,
        &want,
        flags.wait,
        flags.wait_timeout,
        flags.poll_version,
        flags.poll_timeout,
        flags.common.verbose,
    )
    .map_err(|error| format!("integration-rc: {error}"))?;
    Ok(0)
}

/// Re-exported for the dispatch module's `workflow list` inside a named
/// workflow, which is a usage error.
pub const LIST_HINT: &str =
    "`list` is a top-level command — run `devine-utils-ci workflow list` to enumerate ALL named workflows";

#[cfg(test)]
mod tests;
