// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The working-tree half of the release flow: `release-check`,
//! `release-start` and `release-finish` all insist on it.
//!
//! The tree must hold nothing but what CI itself leaves behind: `.vault.lock`
//! files and anything under `dist/` are skipped, so a pipeline that just
//! wrote its artefacts does not fail the release it is validating.

use std::path::Path;

use crate::git;

/// Go's `checkCleanWorkingTree`.
pub(super) fn check(root: &Path) -> Result<(), String> {
    let status = git::run_go(root, &["status", "--porcelain"])
        .map_err(|error| format!("git status failed: {error}"))?;
    if status.trim().is_empty() {
        return Ok(());
    }
    let dirty: Vec<&str> = status
        .trim()
        .split('\n')
        .filter(|line| !line.contains(".vault.lock"))
        .filter(|line| !line.contains("dist/"))
        .collect();
    if dirty.is_empty() {
        return Ok(());
    }
    Err(format!(
        "working tree has {} uncommitted change(s):\n  {}",
        dirty.len(),
        dirty.join("\n  ")
    ))
}
