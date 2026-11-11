// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for `bump-version`: the files it touches, the commit it makes and
//! the refusal it raises when the tree carries somebody else's work.

use std::path::Path;

use tempfile::TempDir;

use super::{
    bump_version, commit_message, is_auto_managed, reject_foreign_changes, sync_legacy_go_const,
    sync_mirrors,
};
use crate::config::Config;
use crate::lint_config::ProjectConfig;
use crate::version::Bump;

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

/// A git repository on `main` with a committed `VERSION` and `Cargo.toml`.
fn repo() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "--initial-branch=main"]);
    git(dir.path(), &["config", "user.name", "Bump Test"]);
    git(
        dir.path(),
        &["config", "user.email", "bump@example.invalid"],
    );
    std::fs::write(dir.path().join("VERSION"), "1.2.3\n").expect("VERSION");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\nversion = \"1.2.3\"\n",
    )
    .expect("Cargo.toml");
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-m", "bump: version 1.2.3"]);
    dir
}

/// The configuration `bump-version` runs with against `dir`.
fn config(dir: &Path) -> Config {
    let mut cfg = Config::new();
    cfg.project_root = dir.to_path_buf();
    cfg
}

#[test]
fn the_commit_message_names_a_promotion_differently() {
    assert_eq!(
        commit_message(Bump::Patch, "1.2.3", "1.2.4"),
        "bump: version 1.2.3 → 1.2.4"
    );
    assert_eq!(
        commit_message(Bump::Promote, "1.2.4-rc.1", "1.2.4"),
        "promote: version 1.2.4-rc.1 → 1.2.4"
    );
}

#[test]
fn auto_managed_lines_cover_renames_and_ignore_everything_else() {
    let managed = vec!["VERSION".to_string(), "Cargo.toml".to_string()];
    assert!(is_auto_managed(" M VERSION", &managed));
    assert!(is_auto_managed("?? Cargo.toml", &managed));
    assert!(is_auto_managed("R  old.txt -> VERSION", &managed));
    assert!(!is_auto_managed("?? notes.txt", &managed));
    assert!(!is_auto_managed("?? .vault.lock", &managed));
    assert!(!is_auto_managed("nonsense", &managed));
}

#[test]
fn a_declared_mirror_that_is_absent_fails_closed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = ProjectConfig {
        version_mirrors: vec!["pkg/version/VERSION".to_string()],
        ..ProjectConfig::default()
    };
    let error = sync_mirrors(dir.path(), &project, "1.2.4").expect_err("missing mirror");
    assert!(error.starts_with("declared mirror \"pkg/version/VERSION\" does not exist at "));
    assert!(error.ends_with("(no silent skip; drift is unsafe)"));
}

#[test]
fn a_declared_mirror_is_written_with_the_root_version() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("pkg/version")).expect("dirs");
    std::fs::write(dir.path().join("pkg/version/VERSION"), "1.2.3\n").expect("mirror");
    let project = ProjectConfig {
        version_mirrors: vec!["pkg/version/VERSION".to_string()],
        ..ProjectConfig::default()
    };
    let applied = sync_mirrors(dir.path(), &project, "1.2.4").expect("mirror");
    assert_eq!(applied, vec!["pkg/version/VERSION".to_string()]);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("pkg/version/VERSION")).expect("read"),
        "1.2.4\n"
    );
}

#[test]
fn the_legacy_go_const_is_synced_only_when_the_pattern_is_there() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(!sync_legacy_go_const(dir.path(), "1.2.4", true).expect("absent"));
    let path = dir.path().join("pkg/version/version.go");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
    std::fs::write(&path, "package version\n\nvar VERSION = \"1.2.3\"\n").expect("go file");
    assert!(sync_legacy_go_const(dir.path(), "1.2.4", true).expect("applied"));
    let updated = std::fs::read_to_string(&path).expect("read");
    assert!(updated.contains("var VERSION = \"1.2.4\""));
}

#[test]
fn a_bump_commits_only_the_files_it_wrote() {
    let dir = repo();
    let mut cfg = config(dir.path());
    cfg.project_root = dir.path().to_path_buf();
    assert_eq!(bump_version(&cfg, "patch").expect("bump"), 0);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("VERSION")).expect("read"),
        "1.2.4\n"
    );
    assert!(std::fs::read_to_string(dir.path().join("Cargo.toml"))
        .expect("read")
        .contains("version = \"1.2.4\""));
    assert_eq!(git(dir.path(), &["status", "--porcelain"]), "");
    let subject = git(dir.path(), &["log", "-1", "--pretty=%s"]);
    assert_eq!(subject, "bump: version 1.2.3 → 1.2.4");
}

#[test]
fn unrelated_work_blocks_the_bump_with_go_words() {
    let dir = repo();
    std::fs::write(dir.path().join("notes.txt"), "x\n").expect("notes");
    let error =
        reject_foreign_changes(dir.path(), &["VERSION".to_string()]).expect_err("foreign change");
    assert_eq!(error, "working tree has non-VERSION changes");
}
