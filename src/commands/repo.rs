// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The repository housekeeping commands: `repo-status` and `cleanup`.

use crate::config::Config;
use crate::git;
use crate::output::Report;

/// The directories `cleanup` removes.
const BUILD_DIRECTORIES: [&str; 6] = ["target", "bin", "dist", "_build", ".go-mod", "coverage"];

/// The files `cleanup` removes.
const BUILD_FILES: [&str; 5] = [
    "coverage.out",
    "coverage.html",
    "coverage.json",
    "junit.xml",
    "devine-utils-ci",
];

/// The `repo-status` command: what does the checkout look like?
pub fn repo_status(cfg: &Config) -> Result<i32, String> {
    let root = cfg.root();
    let mut report = Report::new("repo-status");
    if !git::is_repository(root) {
        report.summary = "repo-status: not a git repository".to_string();
        print!("{}", report.render_text());
        return Ok(0);
    }
    let branch = git::current_branch(root)?;
    let head = git::head_sha(root)?;
    let status = git::status_porcelain(root)?;
    let dirty: Vec<&str> = status.lines().filter(|l| !l.trim().is_empty()).collect();
    let commits = git::recent_commits(root, 5);
    report.extra = Some(serde_json::json!({
        "branch": branch,
        "head": head,
        "dirty": dirty.len(),
        "recent_commits": commits,
    }));
    println!("branch: {branch}");
    println!("head:   {head}");
    if dirty.is_empty() {
        println!("status: clean");
    } else {
        println!("status: {} changed path(s)", dirty.len());
        for line in &dirty {
            println!("  {line}");
        }
    }
    for commit in &commits {
        println!("  {commit}");
    }
    report.summary = format!("repo-status: {branch} at {head}");
    print!("{}", report.render_text());
    Ok(0)
}

/// The `cleanup` command: remove the build artifacts.
///
/// The command never removes a path it did not recognise, and it honours
/// `--dry-run`.
pub fn cleanup(cfg: &Config) -> Result<i32, String> {
    let root = cfg.root();
    let mut report = Report::new("cleanup");
    let mut removed = Vec::new();
    for name in BUILD_DIRECTORIES {
        let path = root.join(name);
        if path.is_dir() {
            remove_path(&path, cfg.dry_run)?;
            removed.push(name.to_string());
        }
    }
    for name in BUILD_FILES {
        let path = root.join(name);
        if path.is_file() {
            remove_path(&path, cfg.dry_run)?;
            removed.push(name.to_string());
        }
    }
    report.extra = Some(serde_json::json!({
        "removed": removed,
        "dry_run": cfg.dry_run,
    }));
    for name in &removed {
        if cfg.dry_run {
            println!("  would remove {name}");
        } else {
            println!("  removed {name}");
        }
    }
    report.summary = if cfg.dry_run {
        format!("cleanup: {} path(s) would be removed", removed.len())
    } else {
        format!("cleanup: {} path(s) removed", removed.len())
    };
    print!("{}", report.render_text());
    Ok(0)
}

/// Removes a build path, or does nothing when the run is a dry run.
fn remove_path(path: &std::path::Path, dry_run: bool) -> Result<(), String> {
    if dry_run {
        return Ok(());
    }
    let result = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    result.map_err(|e| format!("failed to remove {}: {e}", path.display()))
}
