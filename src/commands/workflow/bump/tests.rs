// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the binary-repo half of a release: the changelog marker and
//! the commit + tag, exercised in a throwaway git repository.

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

/// An initialised repository with an identity and one commit.
fn repo(dir: &Path) {
    git(dir, &["init", "--initial-branch=main"]);
    git(dir, &["config", "user.name", "Workflow Test"]);
    git(
        dir,
        &["config", "user.email", "workflow-test@example.invalid"],
    );
    std::fs::write(dir.join("VERSION"), "1.2.3-rc.1\n").unwrap();
    std::fs::write(
        dir.join("CHANGELOG.md"),
        "# Log\n\n## [Unreleased]\n\n- pending\n",
    )
    .unwrap();
    git(dir, &["add", "VERSION", "CHANGELOG.md"]);
    git(dir, &["commit", "-m", "initial"]);
}

#[test]
fn the_rc_marker_lands_under_unreleased() {
    let dir = TempDir::new().unwrap();
    repo(dir.path());
    let (rel, changed) =
        update_rc_changelog(dir.path(), "CHANGELOG.md", "1.2.3-rc.4", "rc").unwrap();
    assert_eq!(rel, "CHANGELOG.md");
    assert!(changed);
    let text = std::fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap();
    let heading = text.find("## [Unreleased]").unwrap();
    let marker = text.find("**rc.4（").expect("rc marker");
    let pending = text.find("- pending").unwrap();
    assert!(heading < marker && marker < pending, "{text}");
    assert!(text.contains("发布候选版本"), "{text}");
    assert_eq!(text.matches("**rc.4（").count(), 1);
}

#[test]
fn an_absent_changelog_is_optional_only_when_not_declared() {
    let dir = TempDir::new().unwrap();
    assert_eq!(
        update_rc_changelog(dir.path(), "", "1.2.3-rc.4", "rc").unwrap(),
        (String::new(), false)
    );
    let error = update_rc_changelog(dir.path(), "CHANGELOG.md", "1.2.3-rc.4", "rc").unwrap_err();
    assert!(error.starts_with("read changelog CHANGELOG.md:"), "{error}");
}

#[test]
fn a_changelog_without_the_heading_fails_closed() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("CHANGELOG.md"), "# Log\n").unwrap();
    let error = update_rc_changelog(dir.path(), "CHANGELOG.md", "1.2.3-rc.4", "rc").unwrap_err();
    assert!(
        error.contains("has no \"## [Unreleased]\" section"),
        "{error}"
    );
}

#[test]
fn a_heading_without_a_trailing_newline_still_gets_its_marker() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("CHANGELOG.md"), "# Log\n\n## [Unreleased]").unwrap();
    let (_, changed) = update_rc_changelog(dir.path(), "CHANGELOG.md", "2.0.0-rc.1", "rc").unwrap();
    assert!(changed);
    let text = std::fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap();
    assert!(text.contains("**rc.1（"), "{text}");
}

#[test]
fn a_promote_never_touches_the_changelog() {
    let dir = TempDir::new().unwrap();
    repo(dir.path());
    let before = std::fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap();
    assert_eq!(
        update_rc_changelog(dir.path(), "CHANGELOG.md", "1.2.3", "promote").unwrap(),
        (String::new(), false)
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap(),
        before
    );
}

#[test]
fn a_bump_writes_every_version_file_commits_and_tags() {
    let dir = TempDir::new().unwrap();
    repo(dir.path());
    std::fs::create_dir_all(dir.path().join("pkg/version")).unwrap();
    std::fs::write(dir.path().join("pkg/version/VERSION"), "1.2.3-rc.1\n").unwrap();

    let mut cfg = default_workflow_config();
    cfg.binary_repo.name = "test-binary".to_string();
    cfg.binary_repo.mirror_files = vec!["pkg/version/VERSION".to_string()];
    let runner = Runner::new(dir.path(), false);

    bump_and_tag(&runner, &cfg, "1.2.3-rc.1", "1.2.3-rc.2", "rc").expect("bump");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("VERSION")).unwrap(),
        "1.2.3-rc.2\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("pkg/version/VERSION")).unwrap(),
        "1.2.3-rc.2\n"
    );
    assert_eq!(
        git(dir.path(), &["log", "-1", "--format=%s"]),
        "bump: version 1.2.3-rc.1 → 1.2.3-rc.2"
    );
    let changed = git(dir.path(), &["show", "--format=", "--name-only", "HEAD"]);
    for file in ["VERSION", "CHANGELOG.md", "pkg/version/VERSION"] {
        assert!(changed.contains(file), "{file} missing from:\n{changed}");
    }
    let message = git(
        dir.path(),
        &[
            "for-each-ref",
            "--format=%(contents)",
            "refs/tags/v1.2.3-rc.2",
        ],
    );
    assert!(message.contains("test-binary 1.2.3-rc.2"), "{message}");
    assert!(
        message.contains("Generated by devine-utils-ci workflow."),
        "{message}"
    );
}

#[test]
fn a_promote_uses_its_own_commit_subject() {
    let dir = TempDir::new().unwrap();
    repo(dir.path());
    let mut cfg = default_workflow_config();
    cfg.binary_repo.changelog_file = "CHANGELOG.md".to_string();
    let runner = Runner::new(dir.path(), false);
    bump_and_tag(&runner, &cfg, "1.2.3-rc.1", "1.2.3", "promote").expect("promote");
    assert_eq!(
        git(dir.path(), &["log", "-1", "--format=%s"]),
        "promote: 1.2.3-rc.1 → 1.2.3"
    );
    let changed = git(dir.path(), &["show", "--format=", "--name-only", "HEAD"]);
    assert!(!changed.contains("CHANGELOG.md"), "{changed}");
    assert_eq!(git(dir.path(), &["tag", "--list", "v1.2.3"]), "v1.2.3");
}

#[test]
fn a_dirty_tree_blocks_the_bump_before_anything_is_written() {
    let dir = TempDir::new().unwrap();
    repo(dir.path());
    std::fs::write(dir.path().join("VERSION"), "9.9.9\n").unwrap();
    let cfg = default_workflow_config();
    let runner = Runner::new(dir.path(), false);
    let error = bump_and_tag(&runner, &cfg, "1.2.3-rc.1", "1.2.3-rc.2", "rc").unwrap_err();
    assert!(
        error.starts_with("working tree has tracked changes:"),
        "{error}"
    );
    assert_eq!(git(dir.path(), &["tag", "--list"]), "");
}

#[test]
fn pushing_without_a_remote_fails_with_the_git_command() {
    let dir = TempDir::new().unwrap();
    repo(dir.path());
    let cfg = default_workflow_config();
    let runner = Runner::new(dir.path(), false);
    let error = push_main_and_tag(&runner, &cfg, "v1.2.3").unwrap_err();
    assert!(error.contains("git push origin HEAD:"), "{error}");
}
