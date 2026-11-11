// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The local helpers `ci-recipe` relies on: gunzip, `mkdir -p` and the
//! post-install version check.

use super::*;

#[test]
fn gunzip_writes_the_destination() {
    let root = tempfile::tempdir().expect("temp dir");
    let destination = root.path().join("out");
    gunzip_stream_to_file(&gzip_bytes(b"payload"), &destination.to_string_lossy())
        .expect("gunzip succeeds");
    assert_eq!(
        std::fs::read(&destination).expect("read back"),
        b"payload".to_vec()
    );
}

#[test]
fn mkdir_all_names_the_component_that_failed() {
    let root = tempfile::tempdir().expect("temp dir");
    let file = root.path().join("plain");
    std::fs::write(&file, b"x").expect("fixture file");
    let target = format!("{}/sub", file.display());

    let error = mkdir_all(&target).expect_err("a file is not a directory");
    assert_eq!(error, format!("mkdir {}: not a directory", file.display()));
}

#[test]
fn mkdir_all_creates_the_whole_chain() {
    let root = tempfile::tempdir().expect("temp dir");
    let target = root.path().join("a/b/c");
    let target = target.to_string_lossy().to_string();

    mkdir_all(&target).expect("mkdir -p");
    assert!(Path::new(&target).is_dir());
    mkdir_all(&target).expect("mkdir -p is idempotent");
}

#[test]
fn verify_accepts_a_matching_version() {
    let root = tempfile::tempdir().expect("temp dir");
    let path = root.path().join("stub");
    std::fs::write(&path, stub_binary("0.2.6")).expect("write stub");
    set_executable(&path);

    retry_busy(|| verify_installed_binary(&path.to_string_lossy(), "0.2.6")).expect("verified");
}

#[test]
fn verify_reports_a_mismatch_with_both_versions() {
    let root = tempfile::tempdir().expect("temp dir");
    let path = root.path().join("stub");
    std::fs::write(&path, stub_binary("0.9.9")).expect("write stub");
    set_executable(&path);

    let error = retry_busy(|| verify_installed_binary(&path.to_string_lossy(), "0.2.6"))
        .expect_err("mismatch");
    assert_eq!(
        error,
        "version mismatch: want=0.2.6 got=0.9.9 (full: devine-utils-ci version 0.9.9)"
    );
}

#[test]
fn verify_reports_a_failing_command() {
    let root = tempfile::tempdir().expect("temp dir");
    let path = root.path().join("stub");
    std::fs::write(&path, b"#!/bin/sh\necho partial output\nexit 3\n").expect("write stub");
    set_executable(&path);

    let error = retry_busy(|| verify_installed_binary(&path.to_string_lossy(), "0.2.6"))
        .expect_err("fails");
    assert_eq!(
        error,
        format!(
            "verify: {} version failed: exit status 3 (output: partial output)",
            path.display()
        )
    );
}

#[test]
fn verify_reports_missing_output() {
    let root = tempfile::tempdir().expect("temp dir");
    let path = root.path().join("stub");
    std::fs::write(&path, b"#!/bin/sh\nexit 0\n").expect("write stub");
    set_executable(&path);

    let error = retry_busy(|| verify_installed_binary(&path.to_string_lossy(), "0.2.6"))
        .expect_err("empty");
    assert_eq!(
        error,
        format!("verify: {} version produced no output", path.display())
    );
}

#[test]
fn verify_reports_a_missing_binary() {
    let root = tempfile::tempdir().expect("temp dir");
    let path = root.path().join("absent");

    let error = verify_installed_binary(&path.to_string_lossy(), "0.2.6").expect_err("missing");
    assert_eq!(
        error,
        format!(
            "verify: {} version failed: fork/exec {}: no such file or directory (output: )",
            path.display(),
            path.display()
        )
    );
}
