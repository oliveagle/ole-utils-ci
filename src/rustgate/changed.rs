// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Which crates a branch touched.
//!
//! `--mode dev` has to be quick, and in a workspace the slow part is
//! compiling and testing crates the branch never touched. This module answers
//! one question — which packages does this branch change? — so the toolchain
//! gates can hand `-p` to cargo instead of rebuilding the world.
//!
//! The answer is deliberately allowed to be "everything": a diff that touches
//! the workspace manifest, a crate's own `Cargo.toml`, a toolchain pin or any
//! file that no crate owns widens the scope back to the whole workspace. A
//! fast run may do more work than it needs to; it must never do less.

use std::collections::BTreeSet;
use std::path::Path;

use toml::Value;

use crate::config::Config;
use crate::git;

/// The files that change how every crate builds.
const WHOLE_REPO: [&str; 6] = [
    "Cargo.toml",
    "Cargo.lock",
    "clippy.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
    "rustfmt.toml",
];

/// The directories a diff never has to look at.
const IGNORED_PREFIXES: [&str; 3] = ["target/", ".git/", "reports/"];

/// The crates a `dev` run has to exercise.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    /// Package names to hand to `-p`, sorted and deduplicated.
    pub crates: Vec<String>,
    /// How many files the diff touched.
    pub files: usize,
}

impl Scope {
    /// Whether the diff touched no crate at all.
    pub fn is_empty(&self) -> bool {
        self.crates.is_empty()
    }

    /// One line describing the scope, for the run's output.
    pub fn describe(&self) -> String {
        if self.crates.is_empty() {
            return format!("no crate changed ({} file(s))", self.files);
        }
        format!(
            "{} crate(s) changed: {}",
            self.crates.len(),
            self.crates.join(", ")
        )
    }
}

/// Whether `path`, relative to the repository root, may change any crate.
enum Owner {
    /// Exactly this package.
    Crate(String),
    /// Everything: the change reaches past one crate.
    Everything,
    /// Nothing a gate has to compile.
    Nothing,
}

/// Works out which crates changed since `--base-branch`.
///
/// `Ok(None)` means the scope could not be narrowed — no git, an unknown base
/// branch, or a change that reaches the whole workspace — and the caller must
/// run everything.
pub fn changed_crates(cfg: &Config) -> Result<Option<Scope>, String> {
    let root = cfg.root();
    if !git::is_repository(root) {
        return Ok(None);
    }
    let Some(base) = merge_base(root, &cfg.base_branch) else {
        return Ok(None);
    };
    let mut paths: BTreeSet<String> = BTreeSet::new();
    for line in git::run(root, &["diff", "--name-only", &base, "HEAD"])?.lines() {
        let name = line.trim();
        if !name.is_empty() {
            paths.insert(name.to_string());
        }
    }
    if let Ok(status) = git::status_porcelain(root) {
        for path in porcelain_paths(&status) {
            paths.insert(path);
        }
    }
    let files = paths.len();
    let mut crates = BTreeSet::new();
    for path in &paths {
        match owner_of(root, path) {
            Owner::Crate(name) => {
                crates.insert(name);
            }
            Owner::Everything => return Ok(None),
            Owner::Nothing => {}
        }
    }
    Ok(Some(Scope {
        crates: crates.into_iter().collect(),
        files,
    }))
}

/// The merge base of `base` and `HEAD`, when git can name one.
fn merge_base(root: &Path, base: &str) -> Option<String> {
    if base.trim().is_empty() {
        return None;
    }
    let sha = git::run(root, &["merge-base", base, "HEAD"]).ok()?;
    if sha.is_empty() {
        None
    } else {
        Some(sha)
    }
}

/// The paths in `git status --porcelain` output, including untracked files.
///
/// The status letters are not at a fixed offset: a cleaned status line such as
/// `M file` has one, a raw one such as ` M file` has two, so the codes are read
/// as a leading token instead of by column.
fn porcelain_paths(status: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in status.lines() {
        let line = line.trim_start();
        let split = line.find(char::is_whitespace).unwrap_or(line.len());
        let codes = &line[..split];
        let mut rest = line[split..].trim_start();
        if codes.len() <= 2 && codes.chars().all(is_status_letter) {
            rest = rest.trim_start();
        } else {
            continue;
        }
        if rest.is_empty() {
            continue;
        }
        // A rename reports `old -> new`; the new name is the one on disk.
        let path = rest.rsplit(" -> ").next().unwrap_or(rest);
        let path = path.trim().trim_matches('"');
        if !path.is_empty() {
            paths.push(path.to_string());
        }
    }
    paths
}

/// Whether `c` is one of the status codes `git status --porcelain` writes.
fn is_status_letter(c: char) -> bool {
    matches!(c, 'M' | 'A' | 'D' | 'R' | 'C' | 'U' | '?' | '!' | 'T')
}

/// Finds the crate that owns one changed file.
fn owner_of(root: &Path, rel: &str) -> Owner {
    if IGNORED_PREFIXES
        .iter()
        .any(|prefix| rel.starts_with(prefix))
    {
        return Owner::Nothing;
    }
    if WHOLE_REPO.contains(&rel) {
        return Owner::Everything;
    }
    let path = root.join(rel);
    let mut current = path.parent();
    while let Some(dir) = current {
        if !dir.starts_with(root) {
            break;
        }
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            if let Some(name) = package_name(&manifest, dir) {
                return Owner::Crate(name);
            }
            // A virtual manifest speaks only for its members, so a file at
            // that level belongs to no crate; keep walking.
        }
        if dir == root {
            break;
        }
        current = dir.parent();
    }
    if is_build_input(rel) {
        // A source file outside every crate can still be compiled by one.
        Owner::Everything
    } else {
        Owner::Nothing
    }
}

/// Whether a file outside every crate still takes part in a build.
fn is_build_input(rel: &str) -> bool {
    rel.ends_with(".rs") || rel.ends_with(".toml") || rel.starts_with("scripts/")
}

/// The package name declared in a manifest, if it declares one.
fn package_name(manifest: &Path, dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(manifest).ok()?;
    let value: Value = toml::from_str(&text).ok()?;
    let package = value.get("package")?;
    match package.get("name").and_then(Value::as_str) {
        Some(name) => Some(name.to_string()),
        // `name.workspace = true` inherits from the workspace manifest, which
        // this parser does not follow; the directory name is the same package
        // name in every workspace that does this.
        None => Some(
            dir.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
        ),
    }
}

#[cfg(test)]
mod tests;
