// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `bump-version` command.
//!
//! The command updates the `VERSION` file and the `Cargo.toml` mirror and
//! creates a single-parent bump commit. It never tags: the tag belongs to
//! `release-finish`, after the branch was merged.

use crate::config::Config;
use crate::git;
use crate::version::Bump;

use super::{cargo, read_version};

/// The `bump-version` command.
pub fn bump_version(cfg: &Config, bump: Bump) -> Result<i32, String> {
    let root = cfg.root();
    let current = read_version(root)?;
    let next = current.next(bump)?;
    let next_text = next.to_string();
    let current_text = current.to_string();

    println!("🔢 Version {current_text} → {next_text}");

    if cfg.dry_run {
        println!("  ℹ dry run — nothing was written or committed");
        return Ok(0);
    }

    let mut managed = vec![super::VERSION_FILE.to_string()];
    write_version_file(root, &next_text)?;
    if cargo::write_version(root, &next_text)? {
        println!("  ✓ Cargo.toml synced to {next_text}");
        managed.push("Cargo.toml".to_string());
    }

    if git::is_repository(root) {
        reject_foreign_changes(root, &managed)?;
        for file in &managed {
            let quoted = file.to_string();
            git::run(root, &["add", &quoted])?;
        }
        let message = if bump == Bump::Promote {
            format!("promote: version {current_text} → {next_text}")
        } else {
            format!("bump: version {current_text} → {next_text}")
        };
        git::run(root, &["commit", "-m", &message])?;
        println!("  ✓ Committed: {message}");
    }

    println!("✅ Version bumped successfully");
    println!();
    println!("Next steps:");
    println!("  git push origin HEAD");
    println!();
    println!("After merge to main, run:");
    println!("  devine-utils-ci release-finish");
    Ok(0)
}

/// Writes the `VERSION` file.
fn write_version_file(root: &std::path::Path, version: &str) -> Result<(), String> {
    let path = root.join(super::VERSION_FILE);
    std::fs::write(&path, format!("{version}\n"))
        .map_err(|e| format!("failed to write {}: {e}", path.display()))
}

/// Refuses to continue when the tree carries changes the bump did not make.
fn reject_foreign_changes(root: &std::path::Path, managed: &[String]) -> Result<(), String> {
    let status = git::status_porcelain(root)?;
    let mut foreign = Vec::new();
    for line in status.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let path = trimmed.split_whitespace().nth(1).unwrap_or("");
        if !managed.iter().any(|m| m == path) {
            foreign.push(trimmed.to_string());
        }
    }
    if foreign.is_empty() {
        return Ok(());
    }
    println!("  ⚠ Uncommitted changes that are not part of the version bump:");
    for line in &foreign {
        println!("    {line}");
    }
    println!("  Commit or stash them first.");
    Err("the working tree has changes that are not part of the version bump".to_string())
}
