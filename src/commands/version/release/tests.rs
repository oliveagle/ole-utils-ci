// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for `release-start` and `release-finish`: which branch each model
//! demands, the bump each one leaves behind, and the marker `release-finish`
//! stamps into every stable tag.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::{release_finish, release_start};
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

/// A clone of a bare `origin` on `main`: committed `VERSION`, the legacy
/// Go-const file the strict path stages, and `origin/main` to compare
/// against. The bare remote lives beside the work tree so it never shows
/// up as an uncommitted change.
struct Fixture {
    _base: TempDir,
    work: PathBuf,
}

impl Fixture {
    fn new() -> Fixture {
        let base = tempfile::tempdir().expect("tempdir");
        let origin = base.path().join("origin.git");
        git(
            base.path(),
            &[
                "init",
                "--bare",
                "--initial-branch=main",
                origin.to_str().expect("utf8"),
            ],
        );
        let work = base.path().join("work");
        std::fs::create_dir(&work).expect("work dir");
        git(&work, &["init", "--initial-branch=main"]);
        git(&work, &["config", "user.name", "Release Test"]);
        git(&work, &["config", "user.email", "release@example.invalid"]);
        std::fs::write(work.join("VERSION"), "1.2.3\n").expect("VERSION");
        let version_go = work.join("pkg/version/version.go");
        std::fs::create_dir_all(version_go.parent().expect("parent")).expect("dirs");
        std::fs::write(&version_go, "package version\n\nvar VERSION = \"1.2.3\"\n")
            .expect("version.go");
        git(&work, &["add", "-A"]);
        git(&work, &["commit", "-m", "chore: seed"]);
        git(
            &work,
            &["remote", "add", "origin", origin.to_str().expect("utf8")],
        );
        git(&work, &["push", "-u", "origin", "main"]);
        Fixture { _base: base, work }
    }

    /// The configuration the release commands run with.
    fn config(&self) -> Config {
        let mut cfg = Config::new();
        cfg.project_root = self.work.clone();
        cfg
    }

    /// Commits a bump and lands it on `origin/main`, the state
    /// `release-finish` refuses to tag without.
    fn merge_bump(&self, next: &str) {
        std::fs::write(self.work.join("VERSION"), format!("{next}\n")).expect("VERSION");
        git(&self.work, &["add", "VERSION"]);
        git(
            &self.work,
            &["commit", "-m", &format!("bump: version 1.2.3 → {next}")],
        );
        git(&self.work, &["push", "origin", "main"]);
    }
}

#[test]
fn release_start_refuses_anything_but_main() {
    let fixture = Fixture::new();
    git(&fixture.work, &["checkout", "-b", "feature/x"]);
    let error = release_start(&fixture.config(), "patch").expect_err("branch");
    assert_eq!(
        error,
        "release must start from main, currently on: feature/x"
    );
}

#[test]
fn release_start_rejects_an_unknown_model_before_touching_git() {
    let fixture = Fixture::new();
    let mut cfg = fixture.config();
    cfg.release_model = "loose".to_string();
    let error = release_start(&cfg, "patch").expect_err("model");
    assert_eq!(
        error,
        "unknown release model \"loose\" — use 'simple' or 'strict'"
    );
}

#[test]
fn release_start_cuts_the_strict_branch_and_commits_the_bump() {
    let fixture = Fixture::new();
    let mut cfg = fixture.config();
    cfg.release_model = "strict".to_string();
    assert_eq!(release_start(&cfg, "patch").expect("start"), 0);
    assert_eq!(
        git(&fixture.work, &["branch", "--show-current"]),
        "chore/release-v1.2.4"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.work.join("VERSION")).expect("VERSION"),
        "1.2.4\n"
    );
    assert!(
        git(&fixture.work, &["log", "-1", "--pretty=%s"]).contains("bump: version 1.2.3 → 1.2.4")
    );
}

#[test]
fn release_start_simple_bumps_main_and_leaves_no_branch() {
    let fixture = Fixture::new();
    assert_eq!(release_start(&fixture.config(), "patch").expect("start"), 0);
    assert_eq!(git(&fixture.work, &["branch", "--show-current"]), "main");
    assert_eq!(
        std::fs::read_to_string(fixture.work.join("VERSION")).expect("VERSION"),
        "1.2.4\n"
    );
    assert!(
        git(&fixture.work, &["log", "-1", "--pretty=%s"]).contains("bump: version 1.2.3 → 1.2.4")
    );
}

#[test]
fn release_finish_wants_the_bump_on_origin_main_first() {
    let fixture = Fixture::new();
    let error = release_finish(&fixture.config()).expect_err("unmerged bump");
    assert!(
        error.contains("no bump commit for version 1.2.3 found on main"),
        "{error}"
    );
}

#[test]
fn release_finish_refuses_the_wrong_branch_for_its_model() {
    let fixture = Fixture::new();
    git(&fixture.work, &["checkout", "-b", "feature/x"]);
    let error = release_finish(&fixture.config()).expect_err("branch");
    assert_eq!(
        error,
        "release-finish (simple model) must run on main, currently on: feature/x"
    );

    let mut strict = fixture.config();
    strict.release_model = "strict".to_string();
    let error = release_finish(&strict).expect_err("strict branch");
    assert_eq!(
        error,
        "release-finish (strict model) must run on release branch, currently on: feature/x"
    );
}

#[test]
fn release_finish_refuses_a_tag_that_already_exists() {
    let fixture = Fixture::new();
    fixture.merge_bump("1.2.4");
    git(&fixture.work, &["tag", "-a", "v1.2.4", "-m", "hand made"]);
    let error = release_finish(&fixture.config()).expect_err("tag exists");
    assert_eq!(error, "tag v1.2.4 already exists");
}

#[test]
fn release_finish_pushes_a_tag_carrying_the_release_marker() {
    let fixture = Fixture::new();
    fixture.merge_bump("1.2.4");
    assert_eq!(release_finish(&fixture.config()).expect("finish"), 0);
    let message = git(&fixture.work, &["tag", "-l", "-n99", "v1.2.4"]);
    assert!(
        message.contains("[released via devine-utils-ci release-finish]"),
        "{message}"
    );
    assert!(message.contains("devine_utils v1.2.4"), "{message}");
    let origin = git(&fixture.work, &["ls-remote", "--tags", "origin"]);
    assert!(origin.contains("refs/tags/v1.2.4"), "{origin}");
}
