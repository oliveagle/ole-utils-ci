// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `hook pre-commit`: everything that must hold before a change becomes a
//! commit.
//!
//! The size checks are the point of the hook: a commit that touches too many
//! files, or carries a blob git will keep forever, is a mistake that is cheap
//! to fix now and expensive to fix once it is pushed.

use std::path::Path;

use crate::output::Diagnostic;

use super::policy::{self, human_bytes, Policy};
use super::staged::{self, StagedFile};
use super::{parse_globals, plural, repo_root, Verdict};

/// Runs `hook pre-commit`.
pub fn pre_commit(args: &[String]) -> Result<i32, String> {
    let (policy, rest) = Policy::parse(args)?;
    let (cfg, positional) = parse_globals(&rest)?;
    if let Some(extra) = positional.first() {
        return Err(format!("pre-commit takes no arguments, got {extra:?}"));
    }
    let root = repo_root(&cfg)?;
    let files = staged::staged_files(&root)?;
    let mut verdict = Verdict::new("hook pre-commit");
    let header = format!(
        "🔒 pre-commit: {} staged {}",
        files.len(),
        plural(files.len(), "file")
    );
    if files.is_empty() {
        verdict.pass("nothing staged — nothing to check");
        return Ok(verdict.finish(&cfg, &header, "pre-commit passed"));
    }
    check_count(&root, &policy, &files, &mut verdict);
    check_sizes(&root, &policy, &files, &mut verdict)?;
    check_secrets(&policy, &files, &mut verdict);
    check_whitespace(&root, &mut verdict);
    let extra = serde_json::json!({
        "staged_files": files.len(),
        "max_files": policy.max_files,
        "max_file_bytes": policy.max_file_bytes,
        "protected_branches": policy.protected,
    });
    Ok(verdict
        .with_extra(extra)
        .finish(&cfg, &header, "pre-commit passed"))
}

/// Refuses a commit that touches more files than the budget allows.
///
/// A merge commit is exempt: it carries every file the merged branch changed,
/// and splitting it is not something the author can do.
fn check_count(root: &Path, policy: &Policy, files: &[StagedFile], verdict: &mut Verdict) {
    if staged::merge_in_progress(root) {
        verdict.pass("merge in progress — the file-count budget does not apply");
        return;
    }
    if files.len() <= policy.max_files {
        verdict.pass(format!(
            "{} {}, within the {}-file budget",
            files.len(),
            plural(files.len(), "file"),
            policy.max_files
        ));
        return;
    }
    let listed: Vec<&str> = files.iter().map(|f| f.path.as_str()).take(10).collect();
    let more = files.len() - listed.len();
    let message = format!(
        "{} files staged, over the {}-file budget — split this into commits that each build",
        files.len(),
        policy.max_files
    );
    let mut line = message.clone();
    line.push_str(&format!("\n    files: {}", listed.join(", ")));
    if more > 0 {
        line.push_str(&format!(" … and {more} more"));
    }
    let mut diagnostic = Diagnostic::error("", "commit-too-large", &message);
    diagnostic.detail = listed.join("\n");
    verdict.deny(diagnostic, line);
}

/// Refuses a staged file larger than the per-file budget.
fn check_sizes(
    root: &Path,
    policy: &Policy,
    files: &[StagedFile],
    verdict: &mut Verdict,
) -> Result<(), String> {
    let mut oversized = Vec::new();
    for file in files.iter().filter(|f| f.exists()) {
        let size = staged::staged_size(root, &file.path)?;
        if size > policy.max_file_bytes {
            oversized.push((file.path.clone(), size));
        }
    }
    if oversized.is_empty() {
        verdict.pass(format!(
            "no staged file over {}",
            human_bytes(policy.max_file_bytes)
        ));
        return Ok(());
    }
    for (path, size) in oversized {
        let message = format!(
            "{path} is {}, over the {} per-file budget — keep large files out of git",
            human_bytes(size),
            human_bytes(policy.max_file_bytes)
        );
        verdict.deny(
            Diagnostic::error(&path, "file-too-large", &message),
            message,
        );
    }
    Ok(())
}

/// Refuses the files that are almost always a leaked credential.
fn check_secrets(policy: &Policy, files: &[StagedFile], verdict: &mut Verdict) {
    let found: Vec<&StagedFile> = files
        .iter()
        .filter(|f| f.exists() && is_secret_path(&f.path))
        .collect();
    if found.is_empty() {
        verdict.pass("no credential-looking file staged");
        return;
    }
    for file in found {
        let message = format!(
            "{} looks like a credential — commit a sample instead and keep the real file out of git",
            file.path
        );
        if policy.block_secrets {
            verdict.deny(
                Diagnostic::error(&file.path, "secret-path", &message),
                message,
            );
        } else {
            verdict.pass(format!("{message} (allowed by --allow-secret-paths)"));
        }
    }
}

/// Surfaces git's own whitespace and conflict-marker report for the index.
fn check_whitespace(root: &Path, verdict: &mut Verdict) {
    let report = match staged::whitespace_report(root) {
        Ok(report) => report,
        Err(e) => {
            verdict.deny(
                Diagnostic::error("", "git", e.clone()),
                format!("git diff --check failed: {e}"),
            );
            return;
        }
    };
    if report.is_empty() {
        verdict.pass("no whitespace error or conflict marker in the staged diff");
        return;
    }
    for line in report.lines() {
        let message = line.trim().to_string();
        if message.is_empty() {
            continue;
        }
        verdict.deny(
            Diagnostic::error("", "staged-whitespace", &message),
            message,
        );
    }
}

/// Whether the file name matches one of the credential patterns.
fn is_secret_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    policy::SECRET_PATHS.contains(&name)
        || policy::SECRET_SUFFIXES
            .iter()
            .any(|suffix| name.ends_with(suffix))
}
