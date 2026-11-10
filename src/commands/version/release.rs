// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `release-start` and `release-finish` commands.

use crate::config::Config;
use crate::git;
use crate::version::{bump_name, tag_for, Bump};

use super::read_version;

/// The `release-start` command: create the release branch and the bump.
///
/// The command stays on the release branch; the merge back to `main` happens
/// in the forge, and `release-finish` then creates the tag.
pub fn release_start(cfg: &Config, bump: Bump) -> Result<i32, String> {
    let root = cfg.root();
    let current = read_version(root)?;
    let next = current.next(bump)?;
    let branch = format!("chore/release-v{next}");
    let _name = bump_name(bump);

    if git::is_repository(root) {
        let on_main = git::current_branch(root).unwrap_or_default();
        if on_main != "main" && on_main != "master" {
            return Err(format!(
                "release-start must run from the main branch, not {on_main}"
            ));
        }
        if !git::is_clean(root) {
            return Err("the working tree has uncommitted changes".to_string());
        }
        if cfg.dry_run {
            println!("  ℹ dry run — branch {branch} was not created");
        } else {
            git::run(root, &["checkout", "-b", &branch])?;
            println!("  ✓ Created branch {branch}");
        }
    }

    super::bump::bump_version(cfg, bump)?;

    println!();
    println!("Next steps:");
    println!("  git push origin {branch}");
    println!("  Open a merge request from {branch} into main.");
    Ok(0)
}

/// The `release-finish` command: tag the merged release.
///
/// The tag is annotated and pushed; a manual `git tag` is prohibited because
/// the release pipeline expects an annotated tag.
pub fn release_finish(cfg: &Config) -> Result<i32, String> {
    let root = cfg.root();
    let version = read_version(root)?;
    let tag = tag_for(&version);

    if git::is_repository(root) {
        let branch = git::current_branch(root).unwrap_or_default();
        if branch != "main" && branch != "master" {
            return Err(format!(
                "release-finish must run from the main branch, not {branch}"
            ));
        }
        if !git::is_clean(root) {
            return Err("the working tree has uncommitted changes".to_string());
        }
        if git::tag_exists(root, &tag) {
            return Err(format!("the tag {tag} already exists"));
        }
        git::run(root, &["tag", "-a", &tag, "-m", &format!("Release {tag}")])?;
        println!("  ✓ Created the annotated tag {tag}");
        git::run(root, &["push", "origin", &tag])?;
        println!("  ✓ Pushed {tag}");
    }

    println!("✅ Release {tag} finished");
    Ok(0)
}
