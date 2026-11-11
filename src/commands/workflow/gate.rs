// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The two release rules the promote flow enforces.
//!
//! 1. `workflow promote` is BLOCKED until integration-01 has verified the
//!    same rc through `/version`:
//!    没有完成集成测试 01 的生产发布测试的，不允许发布正式版本。
//! 2. A successful promote MUST trigger integration-02 immediately:
//!    正式版本发布完成，必须用 integration-02 立刻测试。
//!
//! `/version` is the only authoritative signal here: it reports the binary the
//! app is actually running, which pipeline success alone does not prove.

use crate::commands::pipeline_run;

use super::http::{poll_app_version, probe_app_version, APP_VERSION_POLL_STEP};
use super::model::{TestApp, WorkflowConfig};
use super::registry::Context;
use super::util::{ensure_binary_repo_dir, read_version_from_file};

/// The app that authorises a promote.
pub const GATE_APP_NAME: &str = "devine-app-integration-01-function";

/// The app that must be re-tested right after a promote.
pub const POST_PROMOTE_APP_NAME: &str = "devine-app-integration-02-function";

/// The version to expect from a test app: the binary repo's current VERSION.
///
/// Empty when the repo is not visible from the working directory, in which
/// case callers pass `want=""` (accept any version).
pub fn workflow_want_version(cfg: &WorkflowConfig, ctx: &Context) -> String {
    let Ok(repo_dir) = ensure_binary_repo_dir(cfg, &ctx.cwd) else {
        return String::new();
    };
    read_version_from_file(&repo_dir, &cfg.binary_repo.version_file).unwrap_or_default()
}

/// Triggers an app's `main` pipeline, optionally waits, optionally asserts
/// the app's `/version` reports `want`.
///
/// The trigger reuses the `pipeline-run` machinery, so token resolution and
/// the output formatting stay in one place.
pub fn trigger_and_poll_app(
    app: &TestApp,
    want: &str,
    wait: bool,
    wait_timeout: i64,
    poll_version: bool,
    poll_timeout: i64,
    verbose: bool,
) -> Result<(), String> {
    let mut pr_args: Vec<String> = vec![
        app.project_path.clone(),
        "--ref".to_string(),
        "main".to_string(),
        "--output".to_string(),
        "text".to_string(),
    ];
    if wait || poll_version {
        pr_args.push("--wait".to_string());
        let timeout = if wait_timeout <= 0 { 600 } else { wait_timeout };
        pr_args.push("--wait-timeout".to_string());
        pr_args.push(timeout.to_string());
    }
    pipeline_run::pipeline_run(&pr_args)
        .map_err(|error| format!("trigger pipeline {}: {error}", app.project_path))?;

    if poll_version {
        let poll_timeout = if poll_timeout <= 0 { 600 } else { poll_timeout };
        println!(
            "[workflow] polling {} for app_version == {want:?} (timeout {poll_timeout}s)",
            app.version_url
        );
        let got = poll_app_version(
            &app.version_url,
            want,
            poll_timeout,
            verbose,
            APP_VERSION_POLL_STEP,
        )?;
        println!(
            "[workflow] OK — {want} live on {} (app_version={got})",
            app.name
        );
    }
    Ok(())
}

/// Rule 1: integration-01 must already be serving the rc being promoted.
///
/// A non-200 body that happens to echo `app_version` is not proof the app is
/// serving the rc (a crash-looping pod can still serve a stale body), so the
/// status is checked before the version.
pub fn gate_check_integration01_before_promote(
    cfg: &WorkflowConfig,
    want: &str,
) -> Result<(), String> {
    let app = cfg.find_test_app(GATE_APP_NAME).map_err(|error| {
        format!("release gate: integration-01 app not found in catalog: {error}")
    })?;
    if want.is_empty() {
        return Err(
            "release gate: cannot determine the rc to promote — run from inside \
                    the binary repo with a bumped VERSION, or use --from <version>"
                .to_string(),
        );
    }
    let probe = probe_app_version(&app.version_url);
    if let Some(error) = probe.error {
        return Err(format!(
            "release gate: could not reach {} /version ({}): {error} — \
             cannot verify integration-01 is serving {want}",
            app.name, app.version_url
        ));
    }
    let fix = format!(
        "  Fix:  devine-utils-ci workflow integration-rc --app {GATE_APP_NAME} --poll-version --poll-timeout 600"
    );
    let rule = "  Rule: 没有完成集成测试 01 的生产发布测试的，不允许发布正式版本.";
    if probe.status != 200 {
        return Err(format!(
            "release gate: BLOCKED — {} /version returned HTTP {} (want 200) — the app is not \
             healthy, cannot verify it is serving {want}\n{rule}\n{fix}",
            app.name, probe.status
        ));
    }
    if probe.got != want {
        return Err(format!(
            "release gate: BLOCKED — {} is serving app_version={:?} but you are promoting {want:?}\n{rule}\n{fix}",
            app.name, probe.got
        ));
    }
    println!(
        "[workflow promote] release gate OK — {} is serving {want}",
        app.name
    );
    Ok(())
}

/// Rule 2: trigger integration-02 and poll its `/version` for the stable.
///
/// The app is named, not looked up by role: the rule says integration-02.
/// A catalog without it fails loudly instead of silently skipping the check.
pub fn run_integration02_after_promote(
    cfg: &WorkflowConfig,
    want: &str,
    poll_timeout: i64,
    verbose: bool,
) -> Result<(), String> {
    let app = cfg.find_test_app(POST_PROMOTE_APP_NAME).map_err(|error| {
        format!("post-promote check: integration-02 app not found in catalog: {error}")
    })?;
    let poll_timeout = if poll_timeout <= 0 { 600 } else { poll_timeout };
    println!(
        "[workflow promote] post-promote check — triggering {} + polling /version == {want:?}",
        app.name
    );
    let pr_args: Vec<String> = vec![
        app.project_path.clone(),
        "--ref".to_string(),
        "main".to_string(),
        "--output".to_string(),
        "text".to_string(),
        "--wait".to_string(),
        "--wait-timeout".to_string(),
        "600".to_string(),
    ];
    pipeline_run::pipeline_run(&pr_args)
        .map_err(|error| format!("post-promote check: trigger {}: {error}", app.project_path))?;
    let got = poll_app_version(
        &app.version_url,
        want,
        poll_timeout,
        verbose,
        APP_VERSION_POLL_STEP,
    )
    .map_err(|error| {
        format!(
            "post-promote check: {} never reported app_version={want}: {error}",
            app.name
        )
    })?;
    println!(
        "[workflow promote] OK — {want} is serving {} on {got}",
        app.name
    );
    Ok(())
}

#[cfg(test)]
mod tests;
