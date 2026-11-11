// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `release-start` and `release-finish` commands.
//!
//! Rust port of the Go original's `internal/releasectl.go`: the two halves
//! of the release flow around `bump-version`. `release-start` puts the bump
//! commit where the configured release model wants it — straight on `main`
//! for `simple`, on `chore/release-v<NEXT>` for the legacy `strict` model —
//! and `release-finish` is the only sanctioned way to mint the stable tag,
//! because its annotated message carries [`RELEASE_TAG_MARKER`].

use std::path::Path;

use crate::commands::ci_recipe_util::go_errno;
use crate::commands::validate_release_model;
use crate::config::Config;
use crate::git;
use crate::version::{parse_bump, Version};

use super::{branchcheck, cleantree, releasecheck, RELEASE_TAG_MARKER};

/// The `release-start` command: prepare the bump for the release model.
///
/// `bump` is the raw text of the requested action. Go validates it only
/// after the model, the branch, the tree and the remote have all been
/// checked, so those failures win over an unknown bump type.
pub fn release_start(cfg: &Config, bump: &str) -> Result<i32, String> {
    validate_release_model(&cfg.release_model)?;
    let root = cfg.root();
    let branch = branchcheck::current(root).map_err(|error| format!("not a git repo: {error}"))?;
    if branch != "main" && branch != "master" {
        return Err(format!(
            "release must start from main, currently on: {branch}"
        ));
    }
    cleantree::check(root)?;
    check_main_at_head(root)?;
    if cfg.release_model == "simple" {
        start_simple(cfg, bump)
    } else {
        start_strict(cfg, bump)
    }
}

/// The `release-finish` command: mint and push the release tag.
///
/// The tag is annotated with the marker `release-check` verifies, so a
/// manual `git tag` + `git push` can never pass as a tool-made release.
pub fn release_finish(cfg: &Config) -> Result<i32, String> {
    validate_release_model(&cfg.release_model)?;
    let root = cfg.root();
    let branch = branchcheck::current(root).map_err(|error| format!("not a git repo: {error}"))?;
    if cfg.release_model == "strict" {
        if branch != "release" {
            return Err(format!(
                "release-finish (strict model) must run on release branch, currently on: {branch}"
            ));
        }
        branchcheck::up_to_date(root)
            .map_err(|error| format!("release not up to date with origin/release: {error}"))?;
    } else if branch != "main" && branch != "master" {
        return Err(format!(
            "release-finish (simple model) must run on main, currently on: {branch}"
        ));
    }
    cleantree::check(root)?;
    let version = releasecheck::read_version_file(root)?;
    let tag = format!("v{version}");
    if cfg.release_model != "strict" {
        require_bump_commit(root, &version)?;
    }
    let existing = git::run_go(root, &["tag", "-l", &tag]).unwrap_or_default();
    if !existing.trim().is_empty() {
        return Err(format!("tag {tag} already exists"));
    }
    let message = format!("devine_utils {tag} {RELEASE_TAG_MARKER}");
    git::run_go(root, &["tag", "-a", &tag, "-m", &message])
        .map_err(|error| format!("failed to create tag {tag}: {error}"))?;
    git::run_go(root, &["push", "origin", &tag])
        .map_err(|error| format!("failed to push tag {tag}: {error}"))?;
    println!("release-finish: tag {tag} pushed on {branch}");
    Ok(0)
}

/// `release-start` in the default `simple` model: `BumpVersion` does the
/// write and the commit on `main`, and no branch is cut.
///
/// Go hard-codes the bump's dry-run to false here, so `--dry-run` is read
/// by `bump-version` itself but never reaches this path.
fn start_simple(cfg: &Config, bump: &str) -> Result<i32, String> {
    let current = read_current(cfg.root())?;
    let action = parse_bump(bump)?;
    let next = current.next(action)?;
    println!("  Current: v{current}");
    println!("  Next:    v{next}");
    println!("  Model:   simple (main-only, no release branch)");
    let mut eager = cfg.clone();
    eager.dry_run = false;
    super::bump::bump_version(&eager, bump)?;
    print_simple_next_steps();
    Ok(0)
}

/// `release-start --release-model strict`: the legacy hand-rolled path that
/// cuts `chore/release-v<NEXT>` and commits `VERSION` there itself, because
/// it shipped before `version_mirrors` existed.
fn start_strict(cfg: &Config, bump: &str) -> Result<i32, String> {
    let root = cfg.root();
    let current = read_current(root)?;
    let action = parse_bump(bump)?;
    let next = current.next(action)?;
    let next_text = next.to_string();
    let tag = format!("v{next_text}");
    let branch = format!("chore/release-v{next_text}");
    println!("  Current: v{current}");
    println!("  Next:    {tag}");
    println!("  Branch:  {branch}");

    git::run_go(root, &["checkout", "-b", &branch])
        .map_err(|error| format!("failed to create branch {branch}: {error}"))?;
    std::fs::write(root.join(super::VERSION_FILE), format!("{next_text}\n"))
        .map_err(|error| format!("failed to write VERSION: {error}"))?;
    super::bump::sync_legacy_go_const(root, &next_text, false)?;
    git::run_go(root, &["add", "VERSION", "pkg/version/version.go"])
        .map_err(|error| format!("git add failed: {error}"))?;
    let message = format!("bump: version {current} → {next_text}");
    git::run_go(root, &["commit", "-m", &message])
        .map_err(|error| format!("git commit failed: {error}"))?;

    println!("release-start: v{current} → {tag}, branch {branch}");
    println!(
        "  next: git push -u origin {branch}, MR→main, MR main→release, \
         then release-finish --release-model strict on release"
    );
    Ok(0)
}

/// Go's `releaseStartSimple` trailer, verbatim.
fn print_simple_next_steps() {
    println!();
    println!("release-start (simple) complete: bump commit is on local main, untagged.");
    println!();
    println!("Next steps (no release branch — the bump stays on main):");
    println!("  # Option A: push main directly if you have Maintainer access");
    println!("  git push origin main");
    println!();
    println!("  # Option B: ship the bump via a normal MR (recommended for review)");
    println!("  git push origin HEAD:refs/for/main -o <mr-options...>");
    println!("  # then merge via GitLab UI; release-finish on main will run afterwards");
    println!();
    println!();
    println!("After merge to main, run:");
    println!("  devine-utils-ci release-finish");
    println!("  (manual `git tag` + `git push` for stable tags is prohibited — ADR 0001)");
}

/// A release may only start when local `main` sits exactly on `origin/main`;
/// a missing remote is not an error, there is simply nothing to compare.
fn check_main_at_head(root: &Path) -> Result<(), String> {
    let Ok(main) = git::run_go(root, &["rev-parse", "origin/main"]) else {
        return Ok(());
    };
    let Ok(head) = git::run_go(root, &["rev-parse", "HEAD"]) else {
        return Ok(());
    };
    if head.trim() != main.trim() {
        return Err(
            "local main is not at origin/main — pull first:\n  git pull origin main".to_string(),
        );
    }
    Ok(())
}

/// The `simple` model tags a bump that has already landed on `origin/main`.
fn require_bump_commit(root: &Path, version: &str) -> Result<(), String> {
    let Ok(log) = git::run_go(root, &["log", "--oneline", "-20", "origin/main"]) else {
        return Ok(());
    };
    let landed = log.lines().any(|line| {
        let named = line.contains("bump: version") || line.contains("promote: version");
        named && line.contains(version)
    });
    if landed {
        return Ok(());
    }
    Err(format!(
        "no bump commit for version {version} found on main — merge the bump commit first \
         (e.g. via MR or git push origin main)"
    ))
}

/// Reads `VERSION` with the Go original's wording: `release-start` parses
/// the file itself instead of going through `release-check`'s validator.
fn read_current(root: &Path) -> Result<Version, String> {
    let path = root.join(super::VERSION_FILE);
    let raw = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "VERSION file not found: open {}: {}",
            path.display(),
            go_errno(&error)
        )
    })?;
    Version::parse(raw.trim())
}

#[cfg(test)]
mod tests;
