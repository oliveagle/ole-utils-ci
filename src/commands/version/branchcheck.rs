// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The branch half of the release flow: which branch is checked out, what a
//! release or bump branch promises, and whether it still sits on top of
//! `origin/main`. `release-check` walks it for validation; `release-start`
//! and `release-finish` gate on the same answers.

use std::path::Path;

use crate::git;
use crate::static_regex;

/// Go's `getCurrentBranch`: the checked-out branch, overridden by
/// `CI_COMMIT_REF_NAME` the way a pipeline names the ref it is building.
pub(super) fn current(root: &Path) -> Result<String, String> {
    let output = git::run_go(root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let branch = output.trim().to_string();
    match std::env::var("CI_COMMIT_REF_NAME") {
        Ok(name) if !name.is_empty() => Ok(name),
        _ => Ok(branch),
    }
}

/// The version a `chore/release-vX.Y.Z` or `chore/bump-vX.Y.Z` branch
/// promises, when the branch is one of those two shapes.
pub(super) fn pattern_version(branch: &str) -> Option<String> {
    let pattern = static_regex::compile(r"^chore/(?:release|bump)-v(\d+\.\d+\.\d+)$");
    pattern.captures(branch).map(|found| found[1].to_string())
}

/// Go's `checkBranchUpToDate` for a branch that matched a release pattern.
pub(super) fn on_pattern(
    root: &Path,
    branch_version: &str,
    version: &str,
    expected_tag: &str,
) -> Result<(), String> {
    if branch_version != version {
        return Err(format!(
            "branch version v{branch_version} does not match VERSION={version}"
        ));
    }
    up_to_date(root)?;
    let Ok(existing) = git::run_go(root, &["tag", "-l", expected_tag]) else {
        return Ok(());
    };
    if existing.trim().is_empty() {
        return Ok(());
    }
    points_at_head(root, expected_tag)
}

/// Go's main-history check: the bump commit for this version must already be
/// on `origin/main`, so a release can never be finished before its merge.
pub(super) fn on_main(root: &Path, version: &str) -> Result<(), String> {
    let Ok(log) = git::run_go(root, &["log", "--oneline", "-20", "origin/main"]) else {
        return Ok(());
    };
    let bumped = log
        .lines()
        .any(|line| line.contains("bump: version") && line.contains(version));
    let mentioned = log.lines().any(|line| line.contains(version));
    if bumped || mentioned {
        return Ok(());
    }
    Err(format!(
        "no commit for version {version} in main history — merge release branch first"
    ))
}

/// An existing tag for `expected_tag` must point at the checked-out commit.
fn points_at_head(root: &Path, expected_tag: &str) -> Result<(), String> {
    let tag_commit = git::run_go(root, &["rev-list", "-n", "1", expected_tag])
        .map_err(|_| format!("tag {expected_tag} exists but cannot be resolved"))?;
    let tag_commit = tag_commit.trim();
    let Ok(head) = git::run_go(root, &["rev-parse", "HEAD"]) else {
        return Ok(());
    };
    let head = head.trim();
    if tag_commit == head {
        return Ok(());
    }
    Err(format!(
        "tag {expected_tag} points to {tag_commit}, not HEAD"
    ))
}

/// Go's `checkBranchUpToDate`.
pub(super) fn up_to_date(root: &Path) -> Result<(), String> {
    let head = git::run_go(root, &["rev-parse", "HEAD"])
        .map_err(|error| format!("failed to get HEAD: {error}"))?;
    let head = head.trim();
    let main = match git::run_go(root, &["rev-parse", "origin/main"]) {
        Ok(output) => output,
        Err(_) => {
            println!("  ⚠ Cannot access origin/main (no remote or no fetch yet)");
            println!("    Run: git fetch origin main");
            return Ok(());
        }
    };
    check_merge_base(root, head, main.trim())?;
    report_ahead_behind(root, head)
}

/// The checked-out commit must be built on the tip of `origin/main`.
fn check_merge_base(root: &Path, head: &str, main: &str) -> Result<(), String> {
    let merge_base = git::run_go(root, &["merge-base", head, "origin/main"])
        .map_err(|error| format!("merge-base check failed: {error}"))?;
    let merge_base = merge_base.trim();
    if merge_base == main {
        return Ok(());
    }
    Err(format!(
        "branch is not based on latest origin/main\n  \
         origin/main: {main}\n  \
         merge-base:  {merge_base}\n  \
         Run: git fetch origin main && git rebase origin/main"
    ))
}

/// Being behind `origin/main` fails; being ahead is only reported.
fn report_ahead_behind(root: &Path, head: &str) -> Result<(), String> {
    let behind = format!("{head}..origin/main");
    if let Ok(output) = git::run_go(root, &["rev-list", "--count", &behind]) {
        let count = output.trim();
        if count != "0" {
            return Err(format!(
                "branch is {count} commit(s) BEHIND origin/main — rebase first"
            ));
        }
    }
    let ahead = format!("origin/main..{head}");
    if let Ok(output) = git::run_go(root, &["rev-list", "--count", &ahead]) {
        println!(
            "  Branch is {} commit(s) ahead of origin/main",
            output.trim()
        );
    }
    Ok(())
}
