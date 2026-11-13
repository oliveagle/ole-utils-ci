// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The git questions the hooks ask about the staging area.
//!
//! Everything here is read-only plumbing: list what is staged, ask how large a
//! staged blob is, and run git's own `--check` over the staged diff. Keeping
//! the checks in git means the hook sees exactly what the commit will contain,
//! including the additions of a renamed or partially staged file.

use std::path::Path;
use std::process::Command;

/// One staged entry: the status letter and the path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFile {
    /// The `git diff --name-status` letter (`A`, `M`, `D`, `T`).
    pub status: char,
    /// Path relative to the repository root, with `/` separators.
    pub path: String,
}

impl StagedFile {
    /// Whether the entry still exists in the index.
    pub fn exists(&self) -> bool {
        self.status != 'D'
    }
}

/// The repository root that contains `start`, resolved by git itself.
pub fn toplevel(start: &Path) -> Result<std::path::PathBuf, String> {
    let out = run(start, &["rev-parse", "--show-toplevel"])?;
    let text = String::from_utf8_lossy(&out).trim().to_string();
    if text.is_empty() {
        return Err("not inside a git work tree".to_string());
    }
    Ok(std::path::PathBuf::from(text))
}

/// Everything staged for the next commit, in git's order.
///
/// Renames are reported as a delete plus an add (`--no-renames`), so every
/// entry is exactly one status letter and one path.
pub fn staged_files(root: &Path) -> Result<Vec<StagedFile>, String> {
    let out = run(
        root,
        &["diff", "--cached", "--name-status", "--no-renames", "-z"],
    )?;
    Ok(parse_name_status(&out))
}

/// Parses `git diff --name-status -z`.
///
/// With `-z` git separates the status letter from the path with a NUL as well,
/// so the stream is `A` NUL `path` NUL `M` NUL `path` NUL … . Reading it in
/// pairs is the only parse that survives a path containing a tab.
pub fn parse_name_status(out: &[u8]) -> Vec<StagedFile> {
    let records: Vec<String> = out
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .map(|record| String::from_utf8_lossy(record).to_string())
        .collect();
    let mut files = Vec::new();
    for pair in records.chunks(2) {
        let [status, path] = pair else { continue };
        if let Some(status) = status.chars().next() {
            files.push(StagedFile {
                status,
                path: path.clone(),
            });
        }
    }
    files
}

/// The size of the staged blob for `path`, in bytes.
pub fn staged_size(root: &Path, path: &str) -> Result<u64, String> {
    let spec = format!(":{path}");
    let out = run(root, &["cat-file", "-s", &spec])?;
    String::from_utf8_lossy(&out)
        .trim()
        .parse()
        .map_err(|_| format!("git could not report the size of {path}"))
}

/// git's whitespace and conflict-marker report for the staged diff.
///
/// `git diff --check` exits non-zero when it finds something; its standard
/// output already carries `path:line: message`, so it is passed through as-is.
pub fn whitespace_report(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["diff", "--cached", "--check", "--no-color"])
        .output()
        .map_err(|e| format!("failed to run git diff --check: {e}"))?;
    if output.status.success() {
        return Ok(String::new());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

/// Whether a merge is in progress, which makes the file-count budget moot.
pub fn merge_in_progress(root: &Path) -> bool {
    Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--quiet", "--verify", "MERGE_HEAD"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Runs git in `root` and returns its raw standard output.
fn run(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git {}: {e}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}
