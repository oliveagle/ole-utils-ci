// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `version-check` command: every mirror of `VERSION` must agree.
//!
//! Rust port of the Go original's `internal/versioncheck.go`. Unlike
//! `release-check`, this command does not look at `Cargo.toml` or at the
//! working tree: it reads the root `VERSION`, the optional
//! `pkg/version/version.go`, every file and tree declared in
//! `.devine-utils-ci.yaml`, and then the branch, the release tag and the two
//! heads. A gather failure is returned before a single byte is printed, in
//! every output mode.

use std::path::Path;

use crate::commands::ci_recipe_util::go_errno;
use crate::config::Config;
use crate::lint_config::{self, ProjectConfig};
use crate::static_regex;

use super::VERSION_FILE;

mod render;
mod tree;

/// One declared `version_mirrors` file.
#[derive(Debug, Default)]
pub(super) struct MirrorCheck {
    /// The path as the config declared it, relative to the project root.
    pub path: String,
    /// Whether the file exists.
    pub exists: bool,
    /// Whether its content matches the root `VERSION`.
    pub matched: bool,
    /// `""` on success, `missing` / `mismatch: …` on failure.
    pub note: String,
}

/// One declared `tree_mirrors` destination.
#[derive(Debug, Default)]
pub(super) struct TreeMirrorCheck {
    /// The source tree, relative to the project root.
    pub src: String,
    /// This destination, relative to the project root.
    pub dst: String,
    /// Whether both ends exist.
    pub exists: bool,
    /// Whether the two trees are byte-identical.
    pub matched: bool,
    /// `""` on success, otherwise the note the text report prints.
    pub note: String,
}

/// Everything one `version-check` run learned.
#[derive(Debug, Default)]
pub(super) struct Info {
    /// The root `VERSION`, already validated.
    pub version: String,
    /// Whether `pkg/version/version.go` agrees.
    pub version_go_match: bool,
    /// Why it does not, when it does not.
    pub version_go_note: String,
    /// The checked-out branch; `""` outside a git work tree.
    pub branch: String,
    /// Whether git answered at all.
    pub in_git: bool,
    /// `v<VERSION>` when the branch is `main`, else `""`.
    pub tag_on_release: String,
    /// Whether that tag exists locally.
    pub tag_found: bool,
    /// The full SHA of `HEAD`.
    pub release_head: String,
    /// The full SHA of `origin/main`, when it resolves.
    pub main_head: String,
    /// Whether the two heads are the same commit.
    pub release_is_main: bool,
    /// Whether the version is an `rc` pre-release.
    pub is_rc: bool,
    /// The declared version mirrors, in declaration order.
    pub mirrors: Vec<MirrorCheck>,
    /// The declared tree mirrors, in declaration order.
    pub tree_mirrors: Vec<TreeMirrorCheck>,
    /// Non-fatal notes; only the structured report carries them.
    pub warnings: Vec<String>,
    /// Whether the gather completed.
    pub ok: bool,
}

/// The `version-check` command.
pub fn version_check(cfg: &Config) -> Result<i32, String> {
    let info = gather(cfg)?;
    if cfg.output.is_empty() {
        render::text(&info);
    } else {
        render::structured(cfg, &info);
    }
    Ok(0)
}

/// Collects every fact the report needs, failing before any output.
fn gather(cfg: &Config) -> Result<Info, String> {
    let root = cfg.root();
    let mut info = Info {
        version: read_root_version(root)?,
        ..Info::default()
    };
    if classify(&info.version).is_none() {
        return Err(format!(
            "VERSION file is not a valid version (x.y.z or x.y.z-rc.N): '{}'",
            info.version
        ));
    }
    match check_version_go(root, &info.version) {
        Ok(()) => info.version_go_match = true,
        Err(note) => info.version_go_note = note,
    }
    let project =
        lint_config::load(root).map_err(|error| format!("load project config: {error}"))?;
    let version = info.version.clone();
    check_version_mirrors(root, &version, &project, &mut info)?;
    tree::check(root, &project, &mut info)?;
    gather_git(root, &mut info)?;
    info.ok = true;
    Ok(info)
}

/// Reads the root `VERSION` the way Go reads it: BOM first, then whitespace.
fn read_root_version(root: &Path) -> Result<String, String> {
    let path = root.join(VERSION_FILE);
    let data = std::fs::read(&path).map_err(|error| {
        format!(
            "VERSION file not found: open {}: {}",
            path.display(),
            go_errno(&error)
        )
    })?;
    let text = String::from_utf8_lossy(&data);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    Ok(text.trim().to_string())
}

/// `Some(is_rc)` for a legal `x.y.z` / `x.y.z-rc.N`, `None` for anything else.
fn classify(version: &str) -> Option<bool> {
    let pattern = static_regex::compile(
        r"^(?P<major>[0-9]+)\.(?P<minor>[0-9]+)\.(?P<patch>[0-9]+)(?:-rc\.(?P<rc>[0-9]+))?$",
    );
    let captures = pattern.captures(version)?;
    // Go runs every component through `strconv.Atoi`, which rejects a number
    // outside the machine's `int` range rather than accepting it silently.
    for name in ["major", "minor", "patch"] {
        captures.name(name)?.as_str().parse::<i64>().ok()?;
    }
    if let Some(rc) = captures.name("rc") {
        rc.as_str().parse::<i64>().ok()?;
    }
    Some(captures.name("rc").is_some())
}

/// The optional `pkg/version/version.go` mirror of the version.
fn check_version_go(root: &Path, expected: &str) -> Result<(), String> {
    let path = root.join("pkg/version/version.go");
    let data = std::fs::read(&path).map_err(|error| {
        format!(
            "pkg/version/version.go not found: open {}: {}",
            path.display(),
            go_errno(&error)
        )
    })?;
    let source = String::from_utf8_lossy(&data);
    let pattern = static_regex::compile(r#"var VERSION = "([^"]+)""#);
    let captures = pattern.captures(&source).ok_or_else(|| {
        "pkg/version/version.go does not contain var VERSION = \"...\"".to_string()
    })?;
    let embedded = captures.get(1).map_or("", |found| found.as_str());
    if embedded != expected {
        return Err(format!(
            "pkg/version/version.go has VERSION={embedded} but VERSION file has {expected}"
        ));
    }
    Ok(())
}

/// Every declared version mirror must exist and carry the root version.
fn check_version_mirrors(
    root: &Path,
    version: &str,
    project: &ProjectConfig,
    info: &mut Info,
) -> Result<(), String> {
    for relative in &project.version_mirrors {
        let mut mirror = MirrorCheck {
            path: relative.clone(),
            ..MirrorCheck::default()
        };
        let path = root.join(relative);
        let content = match std::fs::read(&path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                mirror.note = "missing".to_string();
                info.mirrors.push(mirror);
                return Err(format!(
                    "declared mirror {relative:?} does not exist at {} — fix .devine-utils-ci.yaml or create the file (drift is unsafe)",
                    path.display()
                ));
            }
            Err(error) => {
                return Err(format!(
                    "read mirror {}: open {}: {}",
                    path.display(),
                    path.display(),
                    go_errno(&error)
                ));
            }
        };
        mirror.exists = true;
        let got = String::from_utf8_lossy(&content).trim().to_string();
        if got == version {
            mirror.matched = true;
            info.mirrors.push(mirror);
            continue;
        }
        mirror.note = format!("mismatch: got {got:?} want {version:?}");
        info.mirrors.push(mirror);
        return Err(format!(
            "mirror {relative:?} content ({got:?}) does not match root VERSION ({version:?}) — drift"
        ));
    }
    Ok(())
}

/// The git half: branch, the release tag and the two heads.
fn gather_git(root: &Path, info: &mut Info) -> Result<(), String> {
    let Ok(branch) = crate::git::run(root, &["rev-parse", "--abbrev-ref", "HEAD"]) else {
        return Ok(());
    };
    info.in_git = true;
    info.branch = branch;
    if let Ok(ci_branch) = std::env::var("CI_COMMIT_REF_NAME") {
        if !ci_branch.is_empty() {
            info.branch = ci_branch;
        }
    }
    // Go only reaches this once git answered, so a non-repository keeps
    // `is_rc` false rather than classifying a version it never checked.
    info.is_rc = classify(&info.version).unwrap_or(false);
    if info.branch != "main" {
        return Ok(());
    }
    // Tags live on main: the release line is the only line that gets one.
    info.tag_on_release = format!("v{}", info.version);
    let listed = crate::git::run(root, &["tag", "-l", &info.tag_on_release]).unwrap_or_default();
    if listed.trim().is_empty() {
        return Err(format!("tag {} not found on main", info.tag_on_release));
    }
    info.tag_found = true;
    if let Ok(head) = crate::git::run(root, &["rev-parse", "HEAD"]) {
        info.release_head = head;
        if let Ok(origin) = crate::git::run(root, &["rev-parse", "origin/main"]) {
            info.main_head = origin;
            info.release_is_main = info.release_head == info.main_head;
            if !info.release_is_main {
                info.warnings
                    .push("main HEAD != origin/main HEAD".to_string());
            }
        }
    }
    Ok(())
}

/// The `-rc.N` suffix removed, leaving the stable base.
fn strip_rc(version: &str) -> &str {
    version.split_once("-rc.").map_or(version, |(base, _)| base)
}

#[cfg(test)]
mod tests;
