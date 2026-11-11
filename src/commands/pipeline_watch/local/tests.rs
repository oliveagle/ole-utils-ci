// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the local git divergence.

use std::process::Command;

use super::*;

/// Creates a checkout with one commit on `main`.
fn repo(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join(name);
    std::fs::create_dir_all(&root).expect("checkout dir");
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(&root)
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .expect("git runs")
    };
    assert!(git(&["init", "-q", "-b", "main"]).status.success());
    assert!(git(&["config", "user.email", "test@example.com"])
        .status
        .success());
    std::fs::write(root.join("README.md"), "one").expect("write file");
    assert!(git(&["add", "."]).status.success());
    assert!(git(&["commit", "-q", "-m", "one"]).status.success());
    dir
}

#[test]
fn the_checkout_is_named_after_the_last_path_segment() {
    let dir = repo("utils");
    let path = repo_dir(dir.path().to_str().unwrap_or_default(), "devine/utils");
    assert!(path.ends_with("utils"), "{path:?}");
}

#[test]
fn a_row_without_a_checkout_stays_unannotated() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut rows = vec![super::super::rows::RepoRow {
        project: "devine/utils".to_string(),
        ..super::super::rows::RepoRow::default()
    }];
    let count = annotate_local_divergence(&mut rows, dir.path().to_str().unwrap_or_default());
    assert_eq!(count, 0);
    assert!(!rows[0].local_found);
}

#[test]
fn an_empty_root_is_a_no_op() {
    let mut rows = vec![super::super::rows::RepoRow {
        project: "devine/utils".to_string(),
        ..super::super::rows::RepoRow::default()
    }];
    assert_eq!(annotate_local_divergence(&mut rows, "   "), 0);
    assert!(!rows[0].local_found);
}

#[test]
fn a_checkout_without_a_remote_is_not_annotated() {
    let dir = repo("utils");
    let mut rows = vec![super::super::rows::RepoRow {
        project: "devine/utils".to_string(),
        ..super::super::rows::RepoRow::default()
    }];
    let count = annotate_local_divergence(&mut rows, dir.path().to_str().unwrap_or_default());
    assert_eq!(count, 0);
}
