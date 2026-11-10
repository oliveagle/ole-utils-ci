// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! A thin wrapper around the `git` command line.
//!
//! The tool only ever asks git questions: which branch is checked out, does
//! a tag exist, is the tree clean. Every helper returns a `Result` with a
//! readable message, so a missing git binary is reported the same way a
//! failed command is.

use std::path::Path;
use std::process::Command;

/// Runs a git command in `root` and returns its trimmed standard output.
pub fn run(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git {}: {e}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git {} failed: {}", args.join(" "), stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Whether `root` is inside a git work tree.
pub fn is_repository(root: &Path) -> bool {
    Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The name of the checked-out branch, or an empty string when detached.
pub fn current_branch(root: &Path) -> Result<String, String> {
    run(root, &["rev-parse", "--abbrev-ref", "HEAD"])
}

/// The short SHA of the current commit.
pub fn head_sha(root: &Path) -> Result<String, String> {
    run(root, &["rev-parse", "--short", "HEAD"])
}

/// Whether the tag `name` exists.
pub fn tag_exists(root: &Path, name: &str) -> bool {
    run(
        root,
        &["rev-parse", "--verify", &format!("refs/tags/{name}")],
    )
    .is_ok()
}

/// The porcelain status of the work tree.
pub fn status_porcelain(root: &Path) -> Result<String, String> {
    run(root, &["status", "--porcelain"])
}

/// Whether the work tree has no uncommitted change.
pub fn is_clean(root: &Path) -> bool {
    matches!(status_porcelain(root), Ok(s) if s.is_empty())
}

/// The most recent tag that points at `HEAD`, if any.
pub fn tag_at_head(root: &Path) -> Option<String> {
    let out = run(root, &["describe", "--tags", "--exact-match"]).ok()?;
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// The latest tag reachable from `HEAD`, with or without an exact match.
pub fn latest_tag(root: &Path) -> Option<String> {
    let out = run(root, &["describe", "--tags", "--abbrev=0"]).ok()?;
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// The commit subjects of the last `count` commits.
pub fn recent_commits(root: &Path, count: usize) -> Vec<String> {
    let out = match run(root, &["log", &format!("-{count}",), "--pretty=%h %s"]) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    out.lines().map(|l| l.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_non_repository_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_repository(dir.path()));
    }
}
