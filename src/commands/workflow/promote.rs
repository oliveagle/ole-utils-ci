// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `workflow promote`: rc to stable, with both release rules enforced.
//!
//! The gate (integration-01 must already serve the rc) runs BEFORE anything
//! is written, so a blocked promote leaves the repo untouched. The
//! post-promote check (integration-02 must be re-tested) runs AFTER the PROD
//! artifact is live, and its failure does not undo the release — it reports
//! that the release happened but was not confirmed.

use crate::runner::Runner;

use super::bump::{bump_and_tag, push_main_and_tag};
use super::flags::{parse_with, scan_int, Take, WorkflowFlags};
use super::gate::{gate_check_integration01_before_promote, run_integration02_after_promote};
use super::http::{poll_artifactory, ARTIFACTORY_POLL_STEP};
use super::model::WorkflowConfig;
use super::registry::Context;
use super::util::{
    artifactory_url, ensure_binary_repo_dir, git_ensure_on_main, go_slice, next_version,
    read_version_from_file,
};

/// The flags of `workflow promote`.
#[derive(Debug, Clone, Default)]
pub struct PromoteFlags {
    common: WorkflowFlags,
    timeout: i64,
    auto_checkout: bool,
    from: String,
    poll_timeout: i64,
    skip_post_check: bool,
}

fn parse_promote(args: &[String]) -> (PromoteFlags, Vec<String>) {
    let mut flags = PromoteFlags::default();
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
            ("--from", Some(value)) => {
                flags.from = value.to_string();
                Some(Take::Value)
            }
            ("--poll-timeout", Some(value)) => {
                flags.poll_timeout = scan_int(value);
                Some(Take::Value)
            }
            ("--skip-post-check", _) => {
                flags.skip_post_check = true;
                Some(Take::Rest)
            }
            _ => None,
        }
    });
    (flags, rest)
}

/// `workflow promote`: rc to stable on main, push, verify PROD, re-test on
/// integration-02.
pub fn workflow_promote(
    args: &[String],
    cfg: &WorkflowConfig,
    ctx: &Context,
) -> Result<i32, String> {
    let (flags, positional) = parse_promote(args);
    if !positional.is_empty() {
        return Err(format!(
            "promote: unexpected positional args {}",
            go_slice(&positional)
        ));
    }
    let repo_dir = ensure_binary_repo_dir(cfg, &ctx.cwd)?;
    let runner = Runner::new(repo_dir, flags.common.verbose);
    run_promote(&runner, cfg, &flags)
}

fn run_promote(runner: &Runner, cfg: &WorkflowConfig, flags: &PromoteFlags) -> Result<i32, String> {
    let version_file = cfg.binary_repo.version_file.as_str();
    let current = read_version_from_file(&runner.dir, version_file)
        .map_err(|error| format!("promote: read {version_file}: {error}"))?;
    let (next, label) =
        next_version(&current, "promote").map_err(|error| format!("promote: {error}"))?;
    let tag = format!("v{next}");
    // The rc being promoted is the current (pre-bump) version.
    let from = if flags.from.is_empty() {
        current.clone()
    } else {
        flags.from.clone()
    };
    println!(
        "[workflow promote] repo={} current={current} next={next} ({label}) tag={tag}",
        cfg.binary_repo.name
    );
    if flags.common.dry_run {
        return Ok(dry_run(&from, &next, &tag));
    }

    // RELEASE GATE (rule 1): integration-01 must be serving the rc.
    gate_check_integration01_before_promote(cfg, &from)?;
    git_ensure_on_main(runner, &cfg.binary_repo.remote, flags.auto_checkout)
        .map_err(|error| format!("promote: {error}"))?;
    bump_and_tag(runner, cfg, &current, &next, "promote")
        .map_err(|error| format!("promote: bump: {error}"))?;
    push_main_and_tag(runner, cfg, &tag).map_err(|error| format!("promote: push: {error}"))?;

    let prod_url = artifactory_url(cfg, &cfg.artifactory.prod_path, &next);
    let timeout = if flags.timeout <= 0 {
        1800
    } else {
        flags.timeout
    };
    println!("[workflow promote] polling PROD artifactory (timeout {timeout}s): {prod_url}");
    poll_artifactory(
        "PROD",
        &prod_url,
        cfg.artifactory.min_size_b,
        timeout,
        flags.common.verbose,
        ARTIFACTORY_POLL_STEP,
    )?;
    println!(
        "\n[workflow promote] OK — {} promoted to stable {next} on PROD.",
        cfg.binary_repo.name
    );

    // POST-PROMOTE CHECK (rule 2): integration-02 MUST run immediately.
    if flags.skip_post_check {
        println!("[workflow promote] WARNING: --skip-post-check set — integration-02 was NOT run.");
        println!("  Rule: 正式版本发布完成，必须用 integration-02 立刻测试.");
        return Ok(0);
    }
    let post =
        run_integration02_after_promote(cfg, &next, flags.poll_timeout, flags.common.verbose);
    post.map_err(|error| {
        format!(
            "post-promote check FAILED: {error}\n\
             The stable version is live, but integration-02 did not confirm it."
        )
    })?;
    Ok(0)
}

/// Prints the plan and reports success without touching anything.
fn dry_run(from: &str, next: &str, tag: &str) -> i32 {
    println!("[workflow promote] dry-run: would");
    println!(
        "  1. gate-check integration-01 is serving {from} (release rule: \
         没有完成集成测试 01 的生产发布测试的，不允许发布正式版本)"
    );
    println!(
        "  2. checkout main, pull --ff-only, promote to {next}, commit, push main + tag {tag}"
    );
    println!("  3. poll PROD artifactory for {next}");
    println!(
        "  4. post-promote check: integration-02 MUST run immediately \
         (正式版本发布完成，必须用 integration-02 立刻测试)"
    );
    0
}

#[cfg(test)]
mod tests;
