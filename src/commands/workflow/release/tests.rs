// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for `workflow release-rc`, including the end-to-end flow
//! against a bare remote and a loopback artifactory.

mod harness;

use std::sync::atomic::Ordering;

use harness::{config, ctx, git, ArtifactoryServer, Fixture};
use tempfile::TempDir;

use super::*;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn release_rc_syncs_main_bumps_and_publishes_uat_only() {
    let fixture = Fixture::new("0.1.35-rc.1");
    let server = ArtifactoryServer::start();
    let cfg = config(&server.base);
    assert_eq!(
        workflow_release_rc(&args(&["--timeout", "1"]), &cfg, &ctx(&fixture.local)).unwrap(),
        0
    );

    assert_eq!(git(&fixture.local, &["branch", "--show-current"]), "main");
    assert_eq!(
        std::fs::read_to_string(fixture.local.join("VERSION")).unwrap(),
        "0.1.35-rc.2\n"
    );
    assert_eq!(fixture.bare_version(), "0.1.35-rc.2");
    let changer = git(&fixture.local, &["show", "HEAD:CHANGELOG.md"]);
    assert!(changer.contains("**rc.2（"), "{changer}");
    assert_eq!(
        git(&fixture.local, &["tag", "--list", "v0.1.35-rc.2"]),
        "v0.1.35-rc.2"
    );
    assert_eq!(server.uat.load(Ordering::SeqCst), 1);
    assert_eq!(
        server.prod.load(Ordering::SeqCst),
        0,
        "an rc never probes PROD"
    );
    // Keep the updater handle meaningful: the fixture owns both clones.
    assert!(fixture.updater.is_dir() && fixture.root.path().is_dir());
}

#[test]
fn a_dry_run_reads_main_from_the_remote_and_changes_nothing() {
    let fixture = Fixture::new("0.1.35-rc.1");
    let cfg = config("http://127.0.0.1:1");
    assert_eq!(
        workflow_release_rc(&args(&["--dry-run"]), &cfg, &ctx(&fixture.local)).unwrap(),
        0
    );
    assert_eq!(
        git(&fixture.local, &["branch", "--show-current"]),
        "feature/stale-version"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.local.join("VERSION")).unwrap(),
        "9.8.7-rc.3\n"
    );
    assert_eq!(git(&fixture.local, &["tag", "--list"]), "");
}

#[test]
fn a_dry_run_without_a_fetched_remote_ref_explains_the_fetch() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".git")).unwrap();
    std::fs::write(dir.path().join("VERSION"), "1.0.0\n").unwrap();
    git(dir.path(), &["init", "--initial-branch=main"]);
    git(
        dir.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://git.example/test/test-binary.git",
        ],
    );
    let cfg = config("http://127.0.0.1:1");
    let error = workflow_release_rc(&args(&["--dry-run"]), &cfg, &ctx(dir.path())).unwrap_err();
    assert!(
        error.contains("release-rc: read VERSION from origin/main"),
        "{error}"
    );
}

#[test]
fn release_rc_outside_the_binary_repo_is_refused() {
    let dir = TempDir::new().unwrap();
    let cfg = config("http://127.0.0.1:1");
    let error = workflow_release_rc(&args(&[]), &cfg, &ctx(dir.path())).unwrap_err();
    assert!(error.contains("cannot locate the binary repo"), "{error}");
}

#[test]
fn release_rc_rejects_positional_arguments() {
    let dir = TempDir::new().unwrap();
    let cfg = config("http://127.0.0.1:1");
    let error = workflow_release_rc(&args(&["stray"]), &cfg, &ctx(dir.path())).unwrap_err();
    assert_eq!(error, "release-rc: unexpected positional args [stray]");
}

#[test]
fn the_parser_defaults_and_switches() {
    let (flags, rest) = parse_release_rc(&args(&["--timeout", "60", "--no-auto-checkout"]));
    assert_eq!(flags.timeout, 60);
    assert!(!flags.auto_checkout);
    assert!(rest.is_empty());

    let (flags, _) = parse_release_rc(&args(&["--no-auto-checkout", "--auto-checkout"]));
    assert!(flags.auto_checkout, "the last switch wins");

    let (flags, _) = parse_release_rc(&args(&["--timeout", "abc"]));
    assert_eq!(flags.timeout, 0, "a non-numeric timeout reads as 0");
}
