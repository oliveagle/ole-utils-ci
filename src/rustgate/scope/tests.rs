// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the scope helpers.

use super::*;

#[test]
fn a_manifest_without_a_workspace_table_is_a_single_crate() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .expect("manifest");
    assert!(!is_workspace(dir.path()));
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"a\"]\n",
    )
    .expect("manifest");
    assert!(is_workspace(dir.path()));
}

#[test]
fn a_missing_manifest_is_not_a_workspace() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(!is_workspace(dir.path()));
}

#[test]
fn a_ci_run_asks_for_the_whole_workspace_and_a_dev_run_keeps_the_leaf_flags() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"a\"]\n",
    )
    .expect("manifest");
    let mut cfg = Config::new();
    cfg.project_root = dir.path().to_path_buf();
    assert_eq!(
        whole_workspace(&cfg, &["--all-targets"]),
        vec!["--workspace".to_string(), "--all-targets".to_string()]
    );
    assert_eq!(args_of(&["--check", "--all"]), vec!["--check", "--all"]);
}

#[test]
fn a_dev_run_outside_git_cannot_narrow_and_runs_everything() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .expect("manifest");
    let mut cfg = Config::new();
    cfg.project_root = dir.path().to_path_buf();
    cfg.mode = crate::config::Mode::Dev;
    let args = scoped(&cfg, toolchain::Driver::Cargo, &["--all-targets"])
        .expect("args")
        .expect("a scope");
    assert_eq!(args, vec!["--all-targets".to_string()]);
}
