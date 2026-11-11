// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `bump-version` command.
//!
//! Rust port of the Go original's `internal/bumpversion.go`: write the new
//! version everywhere it is mirrored, refuse to commit on top of unrelated
//! work, and leave the tree with exactly one bump commit. `Cargo.toml` is
//! the mirror this crate adds for itself; the Go-const file and every
//! `version_mirrors` entry are the Go original's.

use std::path::Path;

use crate::commands::ci_recipe_util::go_errno;
use crate::config::Config;
use crate::git;
use crate::lint_config::{self, ProjectConfig};
use crate::static_regex;
use crate::version::{bump_name, parse_bump, Bump, Version};

use super::cargo;

/// The `bump-version` command.
///
/// Go prints the banner, loads the project config and reads `VERSION`
/// *before* it validates the bump type, so an unknown action is reported
/// after those steps and never instead of them; `bump` is the raw text.
pub fn bump_version(cfg: &Config, bump: &str) -> Result<i32, String> {
    println!("🏷️  Bumping version...");
    let root = cfg.root();
    let project =
        lint_config::load(root).map_err(|error| format!("load project config: {error}"))?;
    let current = read_version(root)?;
    let bump = parse_bump(bump)?;
    let next = current.next(bump)?;
    let current_text = current.to_string();
    let next_text = next.to_string();
    let tag = format!("v{next_text}");
    let message = commit_message(bump, &current_text, &next_text);

    println!("  Current: v{current_text}");
    println!("  Next:    {tag} ({})", bump_name(bump));
    if !project.version_mirrors.is_empty() {
        println!("  Mirrors: {}", project.version_mirrors.join(", "));
    }
    if cfg.dry_run {
        return dry_run(&project, &current_text, &next_text, &tag, &message);
    }

    println!();
    println!("  Writing VERSION...");
    write_version_file(root, &next_text)?;
    let mut managed = vec![super::VERSION_FILE.to_string()];
    if cargo::write_version(root, &next_text)? {
        println!("  ✓ Cargo.toml synced to {next_text}");
        managed.push("Cargo.toml".to_string());
    }
    if sync_legacy_go_const(root, &next_text, true)? {
        managed.push("pkg/version/version.go".to_string());
    }
    managed.extend(sync_mirrors(root, &project, &next_text)?);
    reject_foreign_changes(root, &managed)?;
    commit(root, &managed, &message)?;
    println!("  ✓ Committed: {message}");
    print_next_steps();
    Ok(0)
}

/// Go's dry-run block: the plan, and nothing written.
fn dry_run(
    project: &ProjectConfig,
    current: &str,
    next: &str,
    tag: &str,
    message: &str,
) -> Result<i32, String> {
    println!();
    println!("  [dry-run] Would write VERSION + mirrors, then commit:");
    println!("    VERSION: {current} → {next}");
    for mirror in &project.version_mirrors {
        println!("    mirror:  {mirror} → {next}");
    }
    println!("    commit:  {message}");
    println!("    tag (after merge to main): {tag}");
    Ok(0)
}

/// The trailer every `bump-version` run leaves behind.
fn print_next_steps() {
    println!();
    println!("✅ Version bumped successfully");
    println!();
    println!("Next steps:");
    println!("  git push origin HEAD");
    println!();
    println!("After merge to main, run:");
    println!("  devine-utils-ci release-finish");
    println!("  (manual `git tag` + `git push` for stable tags is prohibited — ADR 0001)");
}

/// Writes the `VERSION` file.
fn write_version_file(root: &Path, version: &str) -> Result<(), String> {
    let path = root.join(super::VERSION_FILE);
    std::fs::write(&path, format!("{version}\n"))
        .map_err(|error| format!("failed to write VERSION: {error}"))
}

/// Reads `VERSION` with the Go original's wording for a missing file.
fn read_version(root: &Path) -> Result<Version, String> {
    let path = root.join(super::VERSION_FILE);
    let raw = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "VERSION file not found at {}: open {}: {}",
            path.display(),
            path.display(),
            go_errno(&error)
        )
    })?;
    Version::parse(raw.trim())
}

/// Rewrites the legacy `var VERSION = "…"` line when the file carries one,
/// and reports whether it did. `announce` is off on the `release-start`
/// strict path, which keeps Go's silent rewrite.
pub(super) fn sync_legacy_go_const(
    root: &Path,
    next: &str,
    announce: bool,
) -> Result<bool, String> {
    let path = root.join("pkg/version/version.go");
    let Ok(data) = std::fs::read_to_string(&path) else {
        return Ok(false);
    };
    let pattern = static_regex::compile(r#"(var VERSION = ")[^"]*(")"#);
    if !pattern.is_match(&data) {
        return Ok(false);
    }
    let replacement = format!("${{1}}{next}${{2}}");
    let updated = pattern.replace_all(&data, replacement).into_owned();
    std::fs::write(&path, updated)
        .map_err(|error| format!("failed to update pkg/version/version.go: {error}"))?;
    if announce {
        println!("  ✓ pkg/version/version.go synced to {next} (legacy Go-const)");
    }
    Ok(true)
}

/// Writes every declared `version_mirrors` entry, failing closed when one
/// is missing: a silent skip would let the root version drift.
fn sync_mirrors(root: &Path, project: &ProjectConfig, next: &str) -> Result<Vec<String>, String> {
    let mut applied = Vec::new();
    for relative in &project.version_mirrors {
        let path = root.join(relative);
        if !path.exists() {
            return Err(format!(
                "declared mirror \"{relative}\" does not exist at {} — fix \
                 .devine-utils-ci.yaml or create the file (no silent skip; drift is unsafe)",
                path.display()
            ));
        }
        std::fs::write(&path, format!("{next}\n"))
            .map_err(|error| format!("failed to write mirror {relative}: {error}"))?;
        println!("  ✓ mirror synced: {relative} → {next}");
        applied.push(relative.clone());
    }
    Ok(applied)
}

/// Everything the bump itself may have touched: those lines are the commit,
/// anything else means the caller has work in flight.
fn reject_foreign_changes(root: &Path, managed: &[String]) -> Result<(), String> {
    let status = git::run_go(root, &["status", "--porcelain"])
        .map_err(|error| format!("git status failed: {error}"))?;
    let mut foreign = Vec::new();
    for line in status.lines() {
        let line = line.trim();
        if line.is_empty() || line.contains(".vault.lock") {
            continue;
        }
        if !is_auto_managed(line, managed) {
            foreign.push(line.to_string());
        }
    }
    if foreign.is_empty() {
        return Ok(());
    }
    println!("  ⚠  Unstaged/uncommitted changes found (non-VERSION):");
    for line in &foreign {
        println!("    {line}");
    }
    println!("  Commit or stash them first.");
    revert(root, managed);
    Err("working tree has non-VERSION changes".to_string())
}

/// Whether one porcelain line names a file the bump is about to commit; the
/// path column may be either side of a rename.
fn is_auto_managed(line: &str, managed: &[String]) -> bool {
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 2 {
        return false;
    }
    let mut paths = vec![fields[1]];
    if fields.len() >= 4 && fields[2] == "->" {
        paths.push(fields[3]);
    }
    paths
        .iter()
        .any(|path| managed.iter().any(|managed| managed == path))
}

/// Puts the files this run rewrote back, so a rejected bump leaves no stale
/// version sitting on top of the caller's work.
fn revert(root: &Path, managed: &[String]) {
    let mut args: Vec<&str> = vec!["checkout", "--"];
    args.extend(managed.iter().map(String::as_str));
    let _ = git::run_go(root, &args);
}

/// Stages the auto-managed files and makes the single bump commit.
fn commit(root: &Path, managed: &[String], message: &str) -> Result<(), String> {
    let mut args: Vec<&str> = vec!["add"];
    args.extend(managed.iter().map(String::as_str));
    git::run_go(root, &args).map_err(|error| format!("git add failed: {error}"))?;
    git::run_go(root, &["commit", "-m", message])
        .map_err(|error| format!("git commit failed: {error}"))?;
    Ok(())
}

/// Go's `bumpCommitMsg`.
fn commit_message(bump: Bump, current: &str, next: &str) -> String {
    if bump == Bump::Promote {
        format!("promote: version {current} → {next}")
    } else {
        format!("bump: version {current} → {next}")
    }
}

#[cfg(test)]
mod tests;
