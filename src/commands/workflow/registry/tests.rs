// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the configuration layers: `~` expansion, repo discovery,
//! the field-by-field merge, and the precedence chain.

use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::*;

/// Writes `body` to `<dir>/<rel>`, creating parents.
fn write(dir: &Path, rel: &str, body: &str) -> PathBuf {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, body).unwrap();
    path
}

#[test]
fn expand_home_only_rewrites_a_leading_tilde() {
    assert_eq!(
        expand_home("~/.devine/utils/workflow.yaml", "/home/u"),
        PathBuf::from("/home/u/.devine/utils/workflow.yaml")
    );
    assert_eq!(expand_home("~", "/home/u"), PathBuf::from("/home/u"));
    assert_eq!(
        expand_home("/abs/w.yaml", "/home/u"),
        PathBuf::from("/abs/w.yaml")
    );
    assert_eq!(
        expand_home("rel/w.yaml", "/home/u"),
        PathBuf::from("rel/w.yaml")
    );
    assert_eq!(expand_home("", "/home/u"), PathBuf::from(""));
}

#[test]
fn discovery_prefers_the_namespaced_file() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join(".git")).unwrap();
    write(root.path(), "workflow.yaml", "workflows: {}\n");
    write(root.path(), ".devine/workflow.yaml", "workflows: {}\n");
    assert_eq!(
        discover_repo_config(root.path()).unwrap(),
        root.path().join(".devine/workflow.yaml")
    );
}

#[test]
fn discovery_walks_up_but_stops_at_the_git_root() {
    let root = TempDir::new().unwrap();
    fs::create_dir_all(root.path().join(".git")).unwrap();
    let nested = root.path().join("a/b/c");
    fs::create_dir_all(&nested).unwrap();
    write(root.path(), ".devine/workflow.yaml", "workflows: {}\n");

    assert_eq!(
        discover_repo_config(&nested).unwrap(),
        root.path().join(".devine/workflow.yaml"),
        "a committed config at the repo root is found from a nested cwd"
    );

    // A config ABOVE the git root belongs to somebody else and must not leak in.
    let workspace = TempDir::new().unwrap();
    write(workspace.path(), "workflow.yaml", "workflows: {}\n");
    let checkout = workspace.path().join("checkout");
    fs::create_dir_all(checkout.join(".git")).unwrap();
    let inside = checkout.join("sub");
    fs::create_dir_all(&inside).unwrap();
    assert!(
        discover_repo_config(&inside).is_none(),
        "the walk must stop at .git"
    );
}

#[test]
fn a_missing_file_is_a_no_op_and_a_blank_one_is_not_an_error() {
    let mut reg = Registry::builtin();
    let before = reg.clone();
    assert!(!merge_file_into(&mut reg, Path::new("/nonexistent/w.yaml")).unwrap());
    assert_eq!(reg, before);

    let dir = TempDir::new().unwrap();
    let blank = write(dir.path(), "w.yaml", "   \n");
    assert!(merge_file_into(&mut reg, &blank).unwrap());
    assert_eq!(reg, before);
}

#[test]
fn a_new_name_becomes_a_new_workflow() {
    let dir = TempDir::new().unwrap();
    let path = write(
        dir.path(),
        "w.yaml",
        r#"
workflows:
  my-app:
    binary_repo:
      name: my-app
      project: ns/my-app
    test_apps:
      - name: my-dev
        project: faas/my-dev
        role: dev-test
        version_url: http://x/version
"#,
    );
    let mut reg = Registry::builtin();
    assert!(merge_file_into(&mut reg, &path).unwrap());
    let cfg = reg.get("my-app").expect("my-app registered");
    assert_eq!(cfg.name, "my-app");
    assert_eq!(cfg.binary_repo.project_path, "ns/my-app");
    assert_eq!(
        cfg.binary_repo.version_file, "VERSION",
        "defaults backfilled"
    );
    assert_eq!(reg.get(DEFAULT_WORKFLOW_NAME).unwrap().test_apps.len(), 4);
}

#[test]
fn overriding_the_builtin_is_a_partial_merge() {
    let dir = TempDir::new().unwrap();
    let path = write(
        dir.path(),
        "w.yaml",
        "workflows:\n  devine-golang-mcp-app:\n    artifactory: { min_size_b: 2000000 }\n",
    );
    let mut reg = Registry::builtin();
    merge_file_into(&mut reg, &path).unwrap();
    let cfg = reg.get(DEFAULT_WORKFLOW_NAME).unwrap();
    assert_eq!(cfg.artifactory.min_size_b, 2_000_000);
    assert_eq!(cfg.test_apps.len(), 4, "untouched fields survive");
    assert_eq!(cfg.binary_repo.name, "devine-golang-mcp-app");
}

#[test]
fn an_empty_workflow_name_is_rejected() {
    let dir = TempDir::new().unwrap();
    let path = write(
        dir.path(),
        "w.yaml",
        "workflows:\n  \"\":\n    test_apps: []\n",
    );
    let mut reg = Registry::builtin();
    let error = merge_file_into(&mut reg, &path).unwrap_err();
    assert!(error.contains("empty workflow name"), "{error}");
}

#[test]
fn a_malformed_file_names_the_path_it_came_from() {
    let dir = TempDir::new().unwrap();
    let path = write(dir.path(), "w.yaml", "workflows: [not-a-map]\n");
    let mut reg = Registry::builtin();
    let error = merge_file_into(&mut reg, &path).unwrap_err();
    assert!(error.starts_with("parse workflow config "), "{error}");
    assert!(error.contains("w.yaml"), "{error}");
}

#[test]
fn layering_runs_builtin_then_user_then_repo_then_explicit() {
    let root = TempDir::new().unwrap();
    let home = root.path().join("home");
    let repo = root.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();
    let cwd = repo.join("nested");
    fs::create_dir_all(&cwd).unwrap();

    write(
        &home,
        ".devine/utils/workflow.yaml",
        "workflows:\n  app:\n    binary_repo: { name: from-user }\n",
    );
    write(
        &repo,
        ".devine/workflow.yaml",
        "workflows:\n  app:\n    binary_repo: { name: from-repo }\n",
    );
    let explicit = write(
        root.path(),
        "explicit.yaml",
        "workflows:\n  app:\n    binary_repo: { name: from-explicit }\n",
    );

    let ctx = Context {
        cwd: cwd.clone(),
        home: home.display().to_string(),
    };
    let reg = load_layered(&explicit.display().to_string(), &ctx).unwrap();
    assert_eq!(
        reg.get("app").unwrap().binary_repo.name,
        "from-explicit",
        "the explicit --config wins"
    );
    assert!(reg.get(DEFAULT_WORKFLOW_NAME).is_some(), "builtin survives");

    // Without the explicit layer the repo file wins over the user file.
    let reg = load_layered("", &ctx).unwrap();
    assert_eq!(reg.get("app").unwrap().binary_repo.name, "from-repo");
}

#[test]
fn extract_config_flag_reads_both_forms() {
    let cases: [(&[&str], &str); 6] = [
        (&[], ""),
        (&["apps"], ""),
        (&["--config", "/tmp/w.yaml"], "/tmp/w.yaml"),
        (&["--config=/tmp/w.yaml"], "/tmp/w.yaml"),
        (&["list", "-config", "a.yaml"], "a.yaml"),
        (
            &["release-rc", "--timeout", "60", "--config", "x.yaml"],
            "x.yaml",
        ),
    ];
    for (args, want) in cases {
        let owned: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        assert_eq!(extract_config_flag(&owned), want, "{args:?}");
    }
}
