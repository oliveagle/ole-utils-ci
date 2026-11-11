// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for `workflow status`: the PROD version candidates and the local
//! repository snapshot.

use std::path::Path;

use tempfile::TempDir;

use super::*;
use crate::commands::workflow::defaults::default_workflow_config;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

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

/// A repo snapshot with `version` and `tags`.
fn snapshot(version: &str, tags: &[&str]) -> RepoInfo {
    RepoInfo {
        version: version.to_string(),
        branch: "main".to_string(),
        head: "abc1234".to_string(),
        tags: tags.iter().map(|tag| tag.to_string()).collect(),
    }
}

fn flags(values: &[&str]) -> StatusFlags {
    parse_status(&args(values)).0
}

#[test]
fn no_repository_means_no_candidates() {
    assert!(candidate_versions(None).is_empty());
}

#[test]
fn candidates_combine_the_version_and_the_tags_without_duplicates() {
    let repo = snapshot("0.1.20", &["v0.1.21", "v0.1.20", "v0.1.19"]);
    assert_eq!(
        candidate_versions(Some(&repo)),
        ["0.1.19", "0.1.20", "0.1.21"]
    );
}

#[test]
fn candidates_are_capped_at_three_before_sorting() {
    // The Go original de-duplicates, caps at three, then sorts, so the cap
    // follows the tag order (newest first), not the sorted order.
    let repo = snapshot("9.9.9", &["v1.0.0", "v2.0.0", "v3.0.0", "v4.0.0"]);
    assert_eq!(candidate_versions(Some(&repo)), ["1.0.0", "2.0.0", "9.9.9"]);
}

#[test]
fn a_snapshot_without_a_version_still_yields_tag_candidates() {
    let repo = snapshot("", &["v1.2.3"]);
    assert_eq!(candidate_versions(Some(&repo)), ["1.2.3"]);
}

#[test]
fn the_snapshot_reads_version_branch_head_and_the_newest_tags() {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "--initial-branch=main"]);
    git(dir.path(), &["config", "user.name", "Workflow Test"]);
    git(
        dir.path(),
        &["config", "user.email", "workflow-test@example.invalid"],
    );
    std::fs::write(dir.path().join("VERSION"), "0.1.35-rc.2\n").unwrap();
    git(dir.path(), &["add", "VERSION"]);
    git(dir.path(), &["commit", "-m", "initial"]);
    for tag in ["v0.1.33", "v0.1.34", "v0.1.35-rc.1"] {
        git(dir.path(), &["tag", tag]);
    }
    let cfg = default_workflow_config();
    let snapshot = repo_snapshot(dir.path(), &cfg, &flags(&["--tags", "2"]));
    assert_eq!(snapshot.version, "0.1.35-rc.2");
    assert_eq!(snapshot.branch, "main");
    assert!(!snapshot.head.is_empty());
    assert_eq!(snapshot.tags.len(), 2, "the tag limit is honoured");
    assert_eq!(snapshot.tags[0], "v0.1.35-rc.1");
    assert_eq!(snapshot.tags[1], "v0.1.34");
}

#[test]
fn a_non_repository_directory_still_yields_an_empty_snapshot() {
    let dir = TempDir::new().unwrap();
    let cfg = default_workflow_config();
    let snapshot = repo_snapshot(dir.path(), &cfg, &flags(&[]));
    assert!(snapshot.version.is_empty());
    assert!(snapshot.branch.is_empty());
    assert!(snapshot.head.is_empty());
    assert!(snapshot.tags.is_empty());
}

#[test]
fn status_json_runs_without_any_app_or_repository() {
    let dir = TempDir::new().unwrap();
    let mut cfg = default_workflow_config();
    cfg.test_apps.clear();
    let ctx = Context {
        cwd: dir.path().to_path_buf(),
        home: String::new(),
    };
    assert_eq!(
        workflow_status(&args(&["--output", "json"]), &cfg, &ctx).unwrap(),
        0
    );
    assert_eq!(workflow_status(&[], &cfg, &ctx).unwrap(), 0);
}

#[test]
fn status_rejects_positional_arguments() {
    let dir = TempDir::new().unwrap();
    let cfg = default_workflow_config();
    let ctx = Context {
        cwd: dir.path().to_path_buf(),
        home: String::new(),
    };
    let error = workflow_status(&args(&["stray"]), &cfg, &ctx).unwrap_err();
    assert_eq!(error, "status: unexpected positional args [stray]");
}

#[test]
fn an_app_info_omits_a_zero_http_status() {
    let info = AppInfo {
        name: "a".to_string(),
        role: "dev-test".to_string(),
        debug: true,
        app_version: String::new(),
        http_status: 0,
        version_url: "http://x/version".to_string(),
    };
    assert_eq!(
        serde_json::to_string(&info).unwrap(),
        r#"{"name":"a","role":"dev-test","debug":true,"app_version":"","version_url":"http://x/version"}"#
    );
    let info = AppInfo {
        http_status: 503,
        ..info
    };
    assert!(serde_json::to_string(&info)
        .unwrap()
        .contains("\"http_status\":503"));
}

#[test]
fn the_status_payload_skips_absent_sections() {
    let out = StatusOut {
        binary_repo: None,
        artifactory_prod_versions: Vec::new(),
        test_apps: Vec::new(),
    };
    assert_eq!(serde_json::to_string(&out).unwrap(), r#"{"test_apps":[]}"#);
}

#[test]
fn the_tag_flag_defaults_to_three() {
    assert_eq!(parse_status(&[]).0.tags, 3);
    assert_eq!(parse_status(&args(&["--tags", "7"])).0.tags, 7);
    assert_eq!(parse_status(&args(&["--tags", "abc"])).0.tags, 0);
}
