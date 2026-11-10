// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The repository-wide half of [`super::goenv`]: the credential scan.
//!
//! The Go original walks the tree with `filepath.Walk` and matches the
//! `oauth2:<token>` literal from issue §5.3. The scan lives in its own module
//! so neither file grows past the repository's own file-length budget.

use std::path::Path;

use regex::Regex;

use super::goenv::Issue;
use crate::output::Severity;

/// Directories the repository-wide credential scan never descends into.
///
/// This is the Go skip set plus `target`, which is the Rust counterpart of
/// `vendor`/`build` and would otherwise make the scan read every compiled
/// artefact in the tree.
const SKIP_DIRS: [&str; 11] = [
    "vendor",
    "node_modules",
    ".git",
    ".codegraph",
    ".omo",
    ".go-mod",
    ".beads",
    "dist",
    "bin",
    "build",
    "target",
];

/// Extensions the repository-wide credential scan never reads.
///
/// `filepath.Ext` returns the suffix from the *last* dot, so `x.pb.go` yields
/// `.go` and the `.pb.go` entry below never matches — kept for parity with the
/// Go original rather than silently "fixed" here.
const SKIP_EXTENSIONS: [&str; 21] = [
    ".png", ".jpg", ".jpeg", ".gif", ".zip", ".tar", ".gz", ".exe", ".so", ".dylib", ".dll",
    ".bin", ".woff", ".woff2", ".ttf", ".eot", ".ico", ".pb.go", ".sum", ".rlib", ".rmeta",
];

/// The `oauth2:<token>` literal pattern from issue §5.3.
fn oauth2_pattern() -> Regex {
    crate::static_regex::compile(r"oauth2:[A-Za-z0-9_-]{8,}")
}

/// Walks the repository for literal `oauth2:` credentials.
pub(crate) fn scan_hardcoded_credentials(root: &Path) -> Vec<Issue> {
    let pattern = oauth2_pattern();
    let mut issues = Vec::new();
    for entry in walk_sorted(root) {
        let rel = crate::fsutil::relative_slashed(root, &entry);
        if let Some(first) = rel.split('/').next() {
            if SKIP_DIRS.contains(&first) {
                continue;
            }
        }
        let Some(ext) = go_ext(&rel) else {
            continue;
        };
        if ext.is_empty() || SKIP_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }
        let Ok(data) = std::fs::read_to_string(&entry) else {
            continue;
        };
        for (index, line) in data.lines().enumerate() {
            if pattern.is_match(line) {
                issues.push(Issue::at(
                    &rel,
                    index + 1,
                    "hardcoded-oauth2",
                    Severity::Error,
                    "hardcoded oauth2 credential pattern detected".to_string(),
                ));
            }
        }
    }
    issues
}

/// Collects every file under `root` in lexical order, skipping the known
/// generated directories.
fn walk_sorted(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    walk_into(root, &mut out);
    out.sort();
    out
}

fn walk_into(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for item in read.flatten() {
        let path = item.path();
        let Ok(file_type) = item.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            let name = item.file_name().to_string_lossy().to_string();
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            walk_into(&path, out);
        } else if file_type.is_file() {
            out.push(path);
        }
    }
}

/// The `filepath.Ext` equivalent: the suffix from the last dot of the file
/// name, or an empty string when the name has no dot.
fn go_ext(rel: &str) -> Option<String> {
    let name = rel.rsplit('/').next()?;
    match name.rfind('.') {
        Some(index) => Some(name[index..].to_string()),
        None => Some(String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_repo_wide_credential_pattern() {
        let pattern = oauth2_pattern();
        assert!(pattern.is_match("git remote add x https://oauth2:abcd1234ef@git.example"));
        assert!(!pattern.is_match("oauth2:short"));
    }

    #[test]
    fn the_go_extension_rule_looks_at_the_last_dot() {
        assert_eq!(go_ext("a/b/foo.pb.go"), Some(".go".to_string()));
        assert_eq!(go_ext("Makefile"), Some(String::new()));
        assert_eq!(go_ext(".gitignore"), Some(".gitignore".to_string()));
    }
}
