// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Rust port of the Go original's `internal/releasecheck.go`.
//!
//! The checks run in the Go order and stop at the first failure, because
//! that one failure is the whole report: `release-check` has always printed
//! a single error in every output mode, and the structured envelope wraps
//! it rather than collecting the others.

use std::path::Path;

use crate::commands::ci_recipe_util::go_errno;
use crate::commands::validate_release_model;
use crate::config::Config;
use crate::git;
use crate::output::{Diagnostic, Report};
use crate::version::Version;

mod tag;

use super::{branchcheck as branch, cleantree as tree};

/// The `release-check` command.
///
/// Without `--output` the historical contract holds: nothing on success,
/// one `Error:` line on stderr on failure. With a mode selected the same
/// outcome is wrapped in the shared envelope first.
pub(super) fn run(cfg: &Config) -> Result<i32, String> {
    let failure = check(cfg);
    if cfg.output.is_empty() {
        return failure.map(|_| 0);
    }
    let mut payload = Report::new("release-check");
    payload.extra = Some(serde_json::json!({ "release_model": cfg.release_model }));
    let outcome = match &failure {
        Ok(()) => {
            payload.ok = true;
            payload.summary =
                "release-check: release state consistent (VERSION, tag, branch, tree)".to_string();
            Ok(0)
        }
        Err(message) => {
            payload.ok = false;
            payload.summary = "release-check: release state inconsistent".to_string();
            payload.add(Diagnostic::error("", "release-check", message.clone()));
            Err(message.clone())
        }
    };
    print!("{}", payload.render(crate::output::parse_mode(&cfg.output)));
    outcome
}

/// Go's `releaseCheckImpl`: the first broken invariant, or `Ok(())`.
fn check(cfg: &Config) -> Result<(), String> {
    validate_release_model(&cfg.release_model)?;
    let root = cfg.root();
    let version = read_version_file(root)?;
    let expected_tag = format!("v{version}");
    // Go calls `checkVersionGo` here and discards the result, so a drifted
    // `pkg/version/version.go` is deliberately *not* a release-check
    // failure; `version-check` is the command that reports it.
    tree::check(root)?;
    let branch = branch::current(root)
        .map_err(|error| format!("not a git repository — release-check requires git: {error}"))?;
    let ci_tag = std::env::var("CI_COMMIT_TAG").unwrap_or_default();
    if !ci_tag.is_empty() {
        return tagged(root, &cfg.release_model, &version, &expected_tag, &ci_tag);
    }
    match branch::pattern_version(&branch) {
        Some(branch_version) => branch::on_pattern(root, &branch_version, &version, &expected_tag),
        None if branch == "main" || branch == "master" => branch::on_main(root, &version),
        None => Ok(()),
    }?;
    if cfg.release_model != "strict" {
        tag::local_on_main(root, &expected_tag)?;
    }
    Ok(())
}

/// Go's `CI_COMMIT_TAG` path: a tagged pipeline release skips every branch
/// check and only proves the tag sits on the release line.
fn tagged(
    root: &Path,
    model: &str,
    version: &str,
    expected_tag: &str,
    ci_tag: &str,
) -> Result<(), String> {
    if ci_tag != expected_tag {
        return Err(format!("CI_COMMIT_TAG={ci_tag} != VERSION v{version}"));
    }
    let listed = git::run_go(root, &["tag", "-l", ci_tag]).unwrap_or_default();
    if listed.trim() != ci_tag {
        return Err(format!("tag {ci_tag} not found locally"));
    }
    let commit = git::run_go(root, &["rev-list", "-n", "1", ci_tag])
        .map_err(|error| format!("resolve tag {ci_tag}: {error}"))?;
    let release_line = if model == "strict" {
        "origin/release"
    } else {
        "origin/main"
    };
    tag::on_ref(root, ci_tag, commit.trim(), release_line)?;
    tag::marker(root, ci_tag)
}

/// Go's `readVersionFile`, wearing Go's `os.ReadFile` wording.
pub(super) fn read_version_file(root: &Path) -> Result<String, String> {
    let path = root.join("VERSION");
    let raw = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "VERSION file not found: open {}: {}",
            path.display(),
            go_errno(&error)
        )
    })?;
    let text = raw.trim();
    Version::parse(text).map_err(|_| {
        format!("VERSION file contains invalid version: '{text}' (expect x.y.z or x.y.z-rc.N)")
    })?;
    Ok(text.to_string())
}

#[cfg(test)]
mod tests;
