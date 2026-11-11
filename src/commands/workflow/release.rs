// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `workflow release-rc` and `workflow promote`.
//!
//! These own the binary-repo side of the flow: release-rc publishes and
//! verifies the UAT artifact only, while promote is gated on integration-01
//! and re-tests through integration-02. They mirror
//! `scripts/release/release-rc.sh` from devine-golang-mcp-app, but as a
//! command that works for any repo with a VERSION file and a tag-triggered
//! upload chain.

use crate::runner::Runner;

use super::bump::{bump_and_tag, push_main_and_tag};
use super::flags::{parse_with, scan_int, Take, WorkflowFlags};
use super::http::{poll_artifactory, ARTIFACTORY_POLL_STEP};
use super::model::WorkflowConfig;
use super::registry::Context;
use super::util::{
    artifactory_url, ensure_binary_repo_dir, git_ensure_on_main, go_slice, next_version,
    read_version_from_file, run_git,
};

/// The flags of `workflow release-rc`.
#[derive(Debug, Clone)]
pub struct ReleaseRcFlags {
    common: WorkflowFlags,
    timeout: i64,
    auto_checkout: bool,
}

impl Default for ReleaseRcFlags {
    fn default() -> ReleaseRcFlags {
        ReleaseRcFlags {
            common: WorkflowFlags::default(),
            timeout: 0,
            auto_checkout: true,
        }
    }
}

fn parse_release_rc(args: &[String]) -> (ReleaseRcFlags, Vec<String>) {
    let mut flags = ReleaseRcFlags::default();
    let rest = parse_with(args, |arg, next| {
        if let Some(take) = flags.common.take_common(arg, next) {
            return Some(take);
        }
        match (arg, next) {
            ("--timeout", Some(value)) => {
                flags.timeout = scan_int(value);
                Some(Take::Value)
            }
            ("--no-auto-checkout", _) => {
                flags.auto_checkout = false;
                Some(Take::Rest)
            }
            ("--auto-checkout", _) => {
                flags.auto_checkout = true;
                Some(Take::Rest)
            }
            _ => None,
        }
    });
    (flags, rest)
}

/// `workflow release-rc`: bump rc on main, push main + tag, verify UAT.
pub fn workflow_release_rc(
    args: &[String],
    cfg: &WorkflowConfig,
    ctx: &Context,
) -> Result<i32, String> {
    let (flags, positional) = parse_release_rc(args);
    if !positional.is_empty() {
        return Err(format!(
            "release-rc: unexpected positional args {}",
            go_slice(&positional)
        ));
    }
    let repo_dir = ensure_binary_repo_dir(cfg, &ctx.cwd)?;
    let runner = Runner::new(repo_dir, flags.common.verbose);
    run_release_rc(&runner, cfg, &flags)
}

fn run_release_rc(
    runner: &Runner,
    cfg: &WorkflowConfig,
    flags: &ReleaseRcFlags,
) -> Result<i32, String> {
    let version_file = cfg.binary_repo.version_file.as_str();
    let current = if flags.common.dry_run {
        // A dry-run may run from a feature branch, so read the cached remote
        // main ref rather than that branch's VERSION.
        let remote = if cfg.binary_repo.remote.is_empty() {
            "origin"
        } else {
            cfg.binary_repo.remote.as_str()
        };
        let reference = format!("{remote}/main:{version_file}");
        match run_git(runner, &["show", &reference]) {
            Ok(out) => out.trim().to_string(),
            Err(error) => {
                return Err(format!(
                    "release-rc: read {version_file} from {remote}/main (fetch the remote first): {error}"
                ))
            }
        }
    } else {
        // The version on the caller's branch may be stale or unrelated to the
        // main commit that will actually be released.
        git_ensure_on_main(runner, &cfg.binary_repo.remote, flags.auto_checkout)
            .map_err(|error| format!("release-rc: {error}"))?;
        read_version_from_file(&runner.dir, version_file).map_err(|error| {
            format!("release-rc: read {version_file} after syncing main: {error}")
        })?
    };

    let (next, label) =
        next_version(&current, "rc").map_err(|error| format!("release-rc: {error}"))?;
    let tag = format!("v{next}");
    println!(
        "[workflow release-rc] repo={} current={current} next={next} ({label}) tag={tag}",
        cfg.binary_repo.name
    );
    if flags.common.dry_run {
        println!("[workflow release-rc] dry-run: would checkout main, fetch and pull --ff-only,");
        println!(
            "  resolve VERSION on main, bump rc to {next}, update CHANGELOG.md when present, \
             commit, push main + tag {tag}, then poll UAT only"
        );
        return Ok(0);
    }

    bump_and_tag(runner, cfg, &current, &next, "rc")
        .map_err(|error| format!("release-rc: bump: {error}"))?;
    push_main_and_tag(runner, cfg, &tag).map_err(|error| format!("release-rc: push: {error}"))?;

    // Verify UAT only: an rc never waits for, or publishes, a PROD artifact.
    let uat_url = artifactory_url(cfg, &cfg.artifactory.uat_path, &next);
    let timeout = if flags.timeout <= 0 {
        1800
    } else {
        flags.timeout
    };
    println!(
        "[workflow release-rc] polling UAT artifactory (timeout {timeout}s):\n  UAT → {uat_url}"
    );
    poll_artifactory(
        "UAT",
        &uat_url,
        cfg.artifactory.min_size_b,
        timeout,
        flags.common.verbose,
        ARTIFACTORY_POLL_STEP,
    )?;
    println!("[workflow release-rc] UAT artifact live.");
    println!(
        "\n[workflow release-rc] OK — {} {next} published on UAT only.",
        cfg.binary_repo.name
    );
    println!("  next step: devine-utils-ci workflow test-rc");
    Ok(0)
}

#[cfg(test)]
mod tests;
