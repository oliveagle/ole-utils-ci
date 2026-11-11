// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the workflow helpers: version arithmetic, URL building and
//! the version-file accessors. The git tests build a throwaway repository in
//! a temp dir, so nothing outside the sandbox is touched.

use std::path::Path;

use tempfile::TempDir;

use super::*;
use crate::commands::workflow::defaults::default_workflow_config;

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

/// A git repo with one commit and the identity configured.
fn repo(dir: &Path) {
    git(dir, &["init", "--initial-branch=main"]);
    git(dir, &["config", "user.name", "Workflow Test"]);
    git(
        dir,
        &["config", "user.email", "workflow-test@example.invalid"],
    );
}

#[test]
fn next_version_rc_chain() {
    assert_eq!(
        next_version("0.1.34", "rc").unwrap(),
        ("0.1.35-rc.1".to_string(), "rc".to_string())
    );
    assert_eq!(
        next_version("0.1.35-rc.1", "rc").unwrap(),
        ("0.1.35-rc.2".to_string(), "rc".to_string())
    );
    assert_eq!(next_version("0.1.35-rc.9", "rc").unwrap().0, "0.1.35-rc.10");
}

#[test]
fn next_version_promote_drops_the_rc_suffix() {
    assert_eq!(
        next_version("0.1.35-rc.2", "promote").unwrap(),
        ("0.1.35".to_string(), "promote".to_string())
    );
}

#[test]
fn next_version_promote_refuses_a_stable_version() {
    let error = next_version("1.0.0", "promote").unwrap_err();
    assert_eq!(
        error,
        "current version \"1.0.0\" is already stable — nothing to promote"
    );
}

#[test]
fn next_version_rejects_unknown_bumps_and_bad_input() {
    let error = next_version("1.0.0", "minor").unwrap_err();
    assert!(error.contains("unknown bump type \"minor\""), "{error}");
    assert!(next_version("nope", "rc").is_err());
}

#[test]
fn artifactory_url_trims_redundant_slashes() {
    let mut cfg = default_workflow_config();
    cfg.artifactory.base_url = "http://art.example/artifactory/".to_string();
    assert_eq!(
        artifactory_url(&cfg, "/repo/uat/", "1.2.3"),
        "http://art.example/artifactory/repo/uat/1.2.3/devine-golang-mcp-app"
    );
}

#[test]
fn go_slice_renders_without_quotes() {
    assert_eq!(go_slice(&[]), "[]");
    assert_eq!(go_slice(&["a".to_string(), "b".to_string()]), "[a b]");
}

#[test]
fn first_chars_counts_characters_not_bytes() {
    assert_eq!(first_chars("发布候选", 2), "发布");
    assert_eq!(first_chars("abc", 10), "abc");
    assert_eq!(first_chars("abcdef", 0), "");
}

#[test]
fn non_empty_or_picks_the_first_populated_value() {
    assert_eq!(non_empty_or("", "fallback"), "fallback");
    assert_eq!(non_empty_or("value", "fallback"), "value");
}

#[test]
fn version_file_round_trips_with_a_trailing_newline() {
    let dir = TempDir::new().unwrap();
    write_version_to_file(dir.path(), "VERSION", "1.2.3-rc.4").unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("VERSION")).unwrap(),
        "1.2.3-rc.4\n"
    );
    assert_eq!(
        read_version_from_file(dir.path(), "VERSION").unwrap(),
        "1.2.3-rc.4"
    );
}

#[test]
fn reading_a_missing_version_file_is_an_error() {
    let dir = TempDir::new().unwrap();
    assert!(read_version_from_file(dir.path(), "VERSION").is_err());
}

#[test]
fn binary_repo_detection_needs_the_version_file_and_the_remote() {
    let dir = TempDir::new().unwrap();
    repo(dir.path());
    let cfg = default_workflow_config();
    assert!(!is_binary_repo_dir(dir.path(), &cfg));

    write_version_to_file(dir.path(), "VERSION", "1.0.0").unwrap();
    assert!(!is_binary_repo_dir(dir.path(), &cfg), "no remote yet");

    git(
        dir.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://git.example/devine/devine-golang-mcp-app.git",
        ],
    );
    assert!(is_binary_repo_dir(dir.path(), &cfg));
}

#[test]
fn ensure_binary_repo_dir_explains_where_it_looked() {
    let dir = TempDir::new().unwrap();
    let cfg = default_workflow_config();
    let error = ensure_binary_repo_dir(&cfg, dir.path()).unwrap_err();
    assert!(error.contains("cannot locate the binary repo"));
    assert!(error.contains("devine/devine-golang-mcp-app"));
}

#[test]
fn tracked_changes_fail_the_clean_check_but_untracked_files_do_not() {
    let dir = TempDir::new().unwrap();
    repo(dir.path());
    write_version_to_file(dir.path(), "VERSION", "1.0.0").unwrap();
    git(dir.path(), &["add", "VERSION"]);
    git(dir.path(), &["commit", "-m", "initial"]);
    let runner = Runner::new(dir.path(), false);
    git_repo_clean(&runner).expect("a freshly committed tree is clean");

    std::fs::write(dir.path().join("untracked.txt"), "x").unwrap();
    git_repo_clean(&runner).expect("untracked files are tolerated");

    std::fs::write(dir.path().join("VERSION"), "2.0.0\n").unwrap();
    let error = git_repo_clean(&runner).unwrap_err();
    assert!(
        error.starts_with("working tree has tracked changes:"),
        "{error}"
    );
}

#[test]
fn git_ensure_on_main_refuses_a_side_branch_without_auto_checkout() {
    let dir = TempDir::new().unwrap();
    repo(dir.path());
    std::fs::write(dir.path().join("VERSION"), "1.0.0\n").unwrap();
    git(dir.path(), &["add", "VERSION"]);
    git(dir.path(), &["commit", "-m", "initial"]);
    git(dir.path(), &["checkout", "-b", "feature/x"]);
    let runner = Runner::new(dir.path(), false);
    let error = git_ensure_on_main(&runner, "origin", false).unwrap_err();
    assert!(error.contains("on branch \"feature/x\""), "{error}");
}
