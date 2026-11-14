// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The version and release commands.
//!
//! `VERSION` is the single source of truth. `Cargo.toml` mirrors it, and the
//! git tag of a release is `v<VERSION>`. The commands here keep the three in
//! sync and refuse to release when they disagree.

mod branchcheck;
pub mod bump;
pub mod cargo;
pub mod check;
mod cleantree;
pub mod release;
mod releasecheck;

pub use check::version_check;

use crate::config::Config;
use crate::git;
use crate::version::{tag_for, Version};

/// The name of the version file.
pub const VERSION_FILE: &str = "VERSION";

/// The marker `release-finish` embeds in every stable tag it creates and
/// `release-check` insists on (ADR 0001).
pub const RELEASE_TAG_MARKER: &str = "[released via devine-utils-ci release-finish]";

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
/// The whole check lives in the `releasecheck` module, which follows
/// the Go original's order and stops at the first broken invariant.
pub fn release_check(cfg: &Config) -> Result<i32, String> {
    releasecheck::run(cfg)
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
