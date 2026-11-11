// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The version and release commands.
//!
//! `VERSION` is the single source of truth. `Cargo.toml` mirrors it, and the
//! git tag of a release is `v<VERSION>`. The commands here keep the three in
//! sync and refuse to release when they disagree.

pub mod bump;
pub mod cargo;
pub mod check;
pub mod release;

pub use check::version_check;

use crate::config::Config;
use crate::git;
use crate::output::{Diagnostic, Report};
use crate::version::{tag_for, Version};

/// The name of the version file.
pub const VERSION_FILE: &str = "VERSION";

/// Reads and parses the `VERSION` file of `root`.
pub fn read_version(root: &std::path::Path) -> Result<Version, String> {
    let path = root.join(VERSION_FILE);
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("VERSION file not found at {}: {e}", path.display()))?;
    let text = raw.trim().trim_start_matches('\u{feff}');
    Version::parse(text)
}

/// The state of the version across the repository.
#[derive(Debug, Clone)]
pub struct VersionState {
    /// The version from the `VERSION` file.
    pub version: Version,
    /// The version declared in `Cargo.toml`, when the project has one.
    pub cargo_version: Option<String>,
    /// The checked-out branch.
    pub branch: String,
    /// The short SHA of the current commit.
    pub head: String,
    /// The tag the release is expected to carry.
    pub expected_tag: String,
    /// Whether that tag already exists.
    pub tag_exists: bool,
}

/// Collects the version state of the repository.
pub fn gather(cfg: &Config) -> Result<VersionState, String> {
    let root = cfg.root();
    let version = read_version(root)?;
    let cargo_version = cargo::read_version(root);
    let (branch, head) = if git::is_repository(root) {
        (
            git::current_branch(root).unwrap_or_default(),
            git::head_sha(root).unwrap_or_default(),
        )
    } else {
        (String::new(), String::new())
    };
    let expected_tag = tag_for(&version);
    let tag_exists = git::is_repository(root) && git::tag_exists(root, &expected_tag);
    Ok(VersionState {
        version,
        cargo_version,
        branch,
        head,
        expected_tag,
        tag_exists,
    })
}

/// The `release-check` command: the pre-release gate.
///
/// The version must be valid, the manifest must agree with it, the tree must
/// be clean and the checkout must sit on the release branch.
pub fn release_check(cfg: &Config) -> Result<i32, String> {
    let state = gather(cfg)?;
    let mut report = Report::new("release-check");
    if let Some(cargo_version) = &state.cargo_version {
        if cargo_version != &state.version.to_string() {
            report.add(Diagnostic::error(
                "Cargo.toml",
                "release/version-mismatch",
                format!(
                    "Cargo.toml declares {cargo_version} but VERSION says {}",
                    state.version
                ),
            ));
        }
    }
    if !git::is_repository(cfg.root()) {
        report.add(Diagnostic::error(
            "",
            "release/not-a-repository",
            "the project is not a git repository",
        ));
    } else {
        if state.branch != "main" && state.branch != "master" {
            report.add(Diagnostic::error(
                "",
                "release/wrong-branch",
                format!(
                    "the release must run from the main branch, not {}",
                    state.branch
                ),
            ));
        }
        if !git::is_clean(cfg.root()) {
            report.add(Diagnostic::error(
                "",
                "release/dirty-tree",
                "the working tree has uncommitted changes",
            ));
        }
    }
    report.extra = Some(serde_json::json!({
        "version": state.version.to_string(),
        "tag": state.expected_tag,
        "tag_exists": state.tag_exists,
        "branch": state.branch,
    }));
    // Go keeps the historical silent-on-success behaviour: without
    // `--output` a clean release prints nothing and a broken one is only the
    // `Error:` line on stderr.
    let failure = report
        .diagnostics
        .first()
        .map(|issue| issue.message.clone())
        .unwrap_or_else(|| "release state is inconsistent".to_string());
    if cfg.output.is_empty() {
        if report.ok {
            return Ok(0);
        }
        return Err(failure);
    }
    // With a mode selected Go rebuilds the payload around the one error it
    // returned: its own summary and a single `release-check` diagnostic.
    let mut payload = Report::new("release-check");
    payload.extra = Some(serde_json::json!({ "release_model": cfg.release_model }));
    if report.ok {
        payload.summary =
            "release-check: release state consistent (VERSION, tag, branch, tree)".to_string();
    } else {
        payload.summary = "release-check: release state inconsistent".to_string();
        payload.add(Diagnostic::error("", "release-check", failure.clone()));
    }
    print!("{}", payload.render(crate::output::parse_mode(&cfg.output)));
    if report.ok {
        Ok(0)
    } else {
        Err(failure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_version_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("VERSION"), "1.2.3\n").unwrap();
        let version = read_version(dir.path()).unwrap();
        assert_eq!(version.to_string(), "1.2.3");
    }

    #[test]
    fn rejects_a_missing_version_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_version(dir.path()).is_err());
    }
}
