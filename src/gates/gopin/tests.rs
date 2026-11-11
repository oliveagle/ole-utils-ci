// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `go-pin` tests: one fixture project per shape of drift.

use std::path::Path;

use super::surfaces::{go_image_re, semver_re};
use super::*;

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).expect("fixture file");
}

/// A project whose every surface agrees with `.go-version`.
fn consistent_project(dir: &Path) {
    std::fs::create_dir_all(dir.join("scripts/ci")).expect("mkdir");
    write(&dir.join(".go-version"), "1.21.0\n");
    write(
        &dir.join(".gitlab-ci.yml"),
        "variables:\n  GO_IMAGE: \"registry.example/devine/golang:1.21.0\"\n  GOTOOLCHAIN: local\n",
    );
    write(&dir.join("Dockerfile"), "FROM golang:1.21.0 AS build\n");
    write(
        &dir.join("scripts/ci/require-go.sh"),
        "REQUIRED_GO_VERSION=\"$(cat .go-version)\"\n",
    );
    write(&dir.join("go.mod"), "module example\n\ngo 1.21\n");
}

#[test]
fn recognises_semver_versions() {
    assert!(semver_re().is_match("1.25.11"));
    assert!(!semver_re().is_match("1.25"));
    assert!(!semver_re().is_match("v1.25.11"));
}

#[test]
fn finds_the_go_image() {
    let caps = go_image_re()
        .captures(r#"  GO_IMAGE: "golang:1.25.11""#)
        .expect("image should match");
    assert_eq!(caps.get(1).map(|m| m.as_str()), Some("1.25.11"));
}

#[test]
fn a_consistent_pin_reports_nothing() {
    let temp = tempfile::tempdir().expect("temp dir");
    let dir = temp.path();
    consistent_project(dir);
    let (findings, extra) = check(dir, &Settings::default()).expect("check");
    assert!(findings.is_empty(), "unexpected findings: {findings:?}");
    let extra = extra.expect("extra");
    assert_eq!(extra["version"], "1.21.0");
    assert_eq!(extra["version_file"], ".go-version");
    assert_eq!(
        extra["platform"],
        format!("{}/{}", std::env::consts::OS, go_arch())
    );
}

#[test]
fn drift_is_reported_on_every_surface() {
    let temp = tempfile::tempdir().expect("temp dir");
    let dir = temp.path();
    consistent_project(dir);
    write(
        &dir.join(".gitlab-ci.yml"),
        "variables:\n  GO_IMAGE: \"registry.example/devine/golang:1.22.0\"\n",
    );
    write(&dir.join("Dockerfile"), "FROM golang:1.22.0 AS build\n");
    write(
        &dir.join("scripts/ci/require-go.sh"),
        "REQUIRED_GO_VERSION=\"1.21.0\"\n",
    );
    write(
        &dir.join("go.mod"),
        "module example\n\ngo 1.20\n\ntoolchain go1.21.0\n",
    );
    let (findings, extra) = check(dir, &Settings::default()).expect("check");
    let rules: Vec<&str> = findings.iter().map(|d| d.rule.as_str()).collect();
    for rule in [
        "go-pin/ci-image",
        "go-pin/ci-hardcoded-image",
        "go-pin/gotoolchain",
        "go-pin/docker-image",
        "go-pin/require-go-hardcode",
        "go-pin/require-go-source",
        "go-pin/go-directive",
        "go-pin/toolchain-directive",
    ] {
        assert!(rules.contains(&rule), "missing {rule} in {rules:?}");
    }
    assert!(extra.is_some());
}

#[test]
fn a_missing_pin_blocks_the_other_surfaces() {
    let temp = tempfile::tempdir().expect("temp dir");
    let dir = temp.path();
    let (findings, extra) = check(dir, &Settings::default()).expect("check");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "go-pin/version-missing");
    assert!(extra.is_none());
}

#[test]
fn a_malformed_pin_blocks_the_other_surfaces() {
    let temp = tempfile::tempdir().expect("temp dir");
    let dir = temp.path();
    write(&dir.join(".go-version"), "1.21\n");
    let (findings, extra) = check(dir, &Settings::default()).expect("check");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "go-pin/version-format");
    assert!(extra.is_none());
}
