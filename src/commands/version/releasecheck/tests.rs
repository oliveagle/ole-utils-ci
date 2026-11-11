// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for `release-check`: the words of every failure it can report and
//! the order it reports them in.

use std::path::Path;

use tempfile::TempDir;

use super::super::{branchcheck as branch, cleantree as tree};
use super::{check, read_version_file};
use crate::config::Config;

/// Runs `git <args>` in `dir`, asserting success.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A git repository on `main` with one commit and a matching `VERSION`.
fn repo() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "--initial-branch=main"]);
    git(dir.path(), &["config", "user.name", "Release Check Test"]);
    git(
        dir.path(),
        &["config", "user.email", "release-check@example.invalid"],
    );
    std::fs::write(dir.path().join("VERSION"), "1.2.3\n").expect("VERSION");
    git(dir.path(), &["add", "VERSION"]);
    git(dir.path(), &["commit", "-m", "bump: version 1.2.3"]);
    dir
}

/// The configuration `release-check` runs with against `dir`.
fn config(dir: &Path) -> Config {
    let mut cfg = Config::new();
    cfg.project_root = dir.to_path_buf();
    cfg
}

#[test]
fn a_clean_main_branch_passes() {
    let dir = repo();
    assert!(check(&config(dir.path())).is_ok());
}

#[test]
fn a_dirty_tree_lists_every_file_but_the_ci_artifacts() {
    let dir = repo();
    std::fs::write(dir.path().join("notes.txt"), "x\n").expect("notes");
    std::fs::create_dir(dir.path().join("dist")).expect("dist");
    std::fs::write(dir.path().join("dist/out.txt"), "y\n").expect("artifact");
    std::fs::write(dir.path().join(".vault.lock"), "z\n").expect("lock");
    let error = check(&config(dir.path())).expect_err("dirty tree must fail");
    assert_eq!(
        error,
        "working tree has 1 uncommitted change(s):\n  ?? notes.txt"
    );
}

#[test]
fn only_ci_artifacts_leave_the_tree_clean() {
    let dir = repo();
    std::fs::write(dir.path().join(".vault.lock"), "z\n").expect("lock");
    std::fs::create_dir(dir.path().join("dist")).expect("dist");
    std::fs::write(dir.path().join("dist/out.txt"), "y\n").expect("artifact");
    assert!(check(&config(dir.path())).is_ok());
}

#[test]
fn an_unknown_release_model_is_reported_before_anything_else() {
    let dir = repo();
    let mut cfg = config(dir.path());
    cfg.release_model = "bogus".to_string();
    let error = check(&cfg).expect_err("bad model must fail");
    assert_eq!(
        error,
        "unknown release model \"bogus\" — use 'simple' or 'strict'"
    );
}

#[test]
fn a_missing_version_file_is_worded_the_way_go_words_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let error = read_version_file(dir.path()).expect_err("no VERSION");
    assert_eq!(
        error,
        format!(
            "VERSION file not found: open {}: no such file or directory",
            dir.path().join("VERSION").display()
        )
    );
}

#[test]
fn an_invalid_version_is_worded_the_way_go_words_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("VERSION"), "1.2\n").expect("VERSION");
    let error = read_version_file(dir.path()).expect_err("bad VERSION");
    assert_eq!(
        error,
        "VERSION file contains invalid version: '1.2' (expect x.y.z or x.y.z-rc.N)"
    );
}

#[test]
fn release_and_bump_branches_carry_the_version_they_promise() {
    assert_eq!(
        branch::pattern_version("chore/release-v1.2.3").as_deref(),
        Some("1.2.3")
    );
    assert_eq!(
        branch::pattern_version("chore/bump-v4.5.6").as_deref(),
        Some("4.5.6")
    );
    assert_eq!(branch::pattern_version("chore/release-v1.2.3-rc.1"), None);
    assert_eq!(branch::pattern_version("chore/release-v1.2"), None);
    assert_eq!(branch::pattern_version("main"), None);
}

#[test]
fn a_release_branch_that_disagrees_with_version_fails_first() {
    let dir = repo();
    git(dir.path(), &["checkout", "-b", "chore/release-v9.9.9"]);
    let error = check(&config(dir.path())).expect_err("mismatched branch");
    assert_eq!(error, "branch version v9.9.9 does not match VERSION=1.2.3");
}

#[test]
fn a_feature_branch_is_accepted_the_way_go_accepts_it() {
    let dir = repo();
    git(dir.path(), &["checkout", "-b", "feat/anything"]);
    assert!(check(&config(dir.path())).is_ok());
}

#[test]
fn the_working_tree_check_skips_the_ci_artifacts_go_skips() {
    let dir = repo();
    std::fs::write(dir.path().join(".vault.lock"), "z\n").expect("lock");
    std::fs::create_dir_all(dir.path().join("dist/nested")).expect("dist");
    std::fs::write(dir.path().join("dist/nested/out.txt"), "y\n").expect("artifact");
    assert!(tree::check(dir.path()).is_ok());
}
