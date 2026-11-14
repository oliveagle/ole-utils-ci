// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the `dev` scope.

use std::path::Path;
use std::process::Command;

use super::*;

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(root)
        .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        status.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&status.stderr)
    );
}

fn write(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("dir");
    }
    std::fs::write(path, body).expect("write");
}

fn commit_all(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", message]);
}

/// A workspace with two members, committed on `main`.
fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"alpha\", \"beta\"]\nresolver = \"2\"\n",
    );
    write(
        root,
        "alpha/Cargo.toml",
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\n",
    );
    write(root, "alpha/src/lib.rs", "pub fn a() {}\n");
    write(
        root,
        "beta/Cargo.toml",
        "[package]\nname = \"beta\"\nversion = \"0.1.0\"\n",
    );
    write(root, "beta/src/lib.rs", "pub fn b() {}\n");
    write(root, "README.md", "# demo\n");
    commit_all(root, "init");
    dir
}

/// Puts the work tree on a branch, the way a change actually arrives.
fn branch(root: &Path, name: &str) {
    git(root, &["checkout", "-q", "-b", name]);
}

fn config(root: &Path) -> Config {
    let mut cfg = Config::new();
    cfg.project_root = root.to_path_buf();
    cfg.base_branch = "main".to_string();
    cfg
}

#[test]
fn a_change_in_one_crate_narrows_the_scope_to_it() {
    let dir = workspace();
    let root = dir.path();
    branch(root, "feature");
    write(root, "beta/src/lib.rs", "pub fn b() -> u8 { 1 }\n");
    commit_all(root, "touch beta");
    let scope = changed_crates(&config(root))
        .expect("scope")
        .expect("narrow");
    assert_eq!(scope.crates, vec!["beta".to_string()]);
}

#[test]
fn an_uncommitted_change_counts_too() {
    let dir = workspace();
    let root = dir.path();
    branch(root, "feature");
    write(root, "alpha/src/lib.rs", "pub fn a() -> u8 { 2 }\n");
    let scope = changed_crates(&config(root))
        .expect("scope")
        .expect("narrow");
    assert_eq!(scope.crates, vec!["alpha".to_string()]);
}

#[test]
fn changes_in_both_crates_are_both_kept() {
    let dir = workspace();
    let root = dir.path();
    branch(root, "feature");
    write(root, "alpha/src/lib.rs", "pub fn a() -> u8 { 3 }\n");
    write(root, "beta/src/lib.rs", "pub fn b() -> u8 { 3 }\n");
    commit_all(root, "touch both");
    let scope = changed_crates(&config(root))
        .expect("scope")
        .expect("narrow");
    assert_eq!(scope.crates, vec!["alpha".to_string(), "beta".to_string()]);
}

#[test]
fn a_manifest_change_widens_the_scope_to_the_workspace() {
    let dir = workspace();
    let root = dir.path();
    branch(root, "feature");
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"alpha\", \"beta\"]\nresolver = \"3\"\n",
    );
    assert!(changed_crates(&config(root)).expect("scope").is_none());
}

#[test]
fn a_crate_manifest_change_narrows_to_that_crate() {
    let dir = workspace();
    let root = dir.path();
    branch(root, "feature");
    write(
        root,
        "beta/Cargo.toml",
        "[package]\nname = \"beta\"\nversion = \"0.2.0\"\n",
    );
    let scope = changed_crates(&config(root))
        .expect("scope")
        .expect("narrow");
    assert_eq!(scope.crates, vec!["beta".to_string()]);
}

#[test]
fn a_source_file_outside_every_crate_widens_the_scope() {
    let dir = workspace();
    let root = dir.path();
    branch(root, "feature");
    write(root, "scripts/helper.rs", "fn main() {}\n");
    assert!(changed_crates(&config(root)).expect("scope").is_none());
}

#[test]
fn a_document_change_touches_no_crate() {
    let dir = workspace();
    let root = dir.path();
    branch(root, "feature");
    write(root, "README.md", "# demo, edited\n");
    commit_all(root, "docs");
    let scope = changed_crates(&config(root))
        .expect("scope")
        .expect("narrow");
    assert!(scope.is_empty());
    assert_eq!(scope.files, 1);
}

#[test]
fn an_unknown_base_branch_cannot_narrow_the_scope() {
    let dir = workspace();
    let root = dir.path();
    let mut cfg = config(root);
    cfg.base_branch = "no-such-branch".to_string();
    assert!(changed_crates(&cfg).expect("scope").is_none());
}

#[test]
fn a_repository_without_its_base_branch_cannot_narrow_the_scope() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    git(root, &["init", "-q", "-b", "trunk"]);
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"solo\"\nversion = \"0.1.0\"\n",
    );
    write(root, "src/lib.rs", "pub fn solo() {}\n");
    commit_all(root, "init");
    assert!(changed_crates(&config(root)).expect("scope").is_none());
}

#[test]
fn porcelain_paths_reads_the_trimmed_and_the_raw_spelling() {
    let raw = " M alpha/src/lib.rs\n?? beta/src/new.rs\nR  old.rs -> new.rs\n";
    let trimmed = "M alpha/src/lib.rs\n?? beta/src/new.rs\n";
    let expected = vec![
        "alpha/src/lib.rs".to_string(),
        "beta/src/new.rs".to_string(),
        "new.rs".to_string(),
    ];
    assert_eq!(porcelain_paths(raw), expected);
    assert_eq!(
        porcelain_paths(trimmed),
        vec![
            "alpha/src/lib.rs".to_string(),
            "beta/src/new.rs".to_string()
        ]
    );
}

#[test]
fn the_scope_describes_itself() {
    let scope = Scope {
        crates: vec!["alpha".to_string(), "beta".to_string()],
        files: 3,
    };
    assert_eq!(scope.describe(), "2 crate(s) changed: alpha, beta");
    assert_eq!(Scope::default().describe(), "no crate changed (0 file(s))");
}
