// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `rhai` group.
//!
//! The gate runs the real Rhai compiler inside the tool, so the tests need
//! no toolchain of their own: the source is compiled, the declared cases are
//! called in a sandbox, and the coverage the debugger observed is reported.

use std::process::Command;

/// Runs a binary with `args` and returns its exit code, stdout and stderr.
fn run(binary: &str, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(binary)
        .args(args)
        .output()
        .expect("failed to spawn the binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Runs the main binary.
fn main_binary(args: &[&str]) -> (i32, String, String) {
    run(env!("CARGO_BIN_EXE_devine-utils-ci"), args)
}

#[test]
fn the_rhai_gate_runs_a_declared_case_suite() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("kernel.rhai"),
        "fn choose(x) { if (x) { 1 } else { 2 } }\n",
    )
    .expect("script write");
    let cases = dir.path().join("cases.json");
    std::fs::write(
        &cases,
        r#"[{"file":"kernel.rhai","function":"choose","args":[true],"expected":1},
            {"file":"kernel.rhai","function":"choose","args":[false],"expected":2}]"#,
    )
    .expect("case write");
    let (code, stdout, stderr) = main_binary(&[
        "rhai-check",
        "--src",
        dir.path().to_string_lossy().as_ref(),
        "--cases",
        cases.to_string_lossy().as_ref(),
        "--output",
        "json",
    ]);
    assert_eq!(code, 0, "stdout: {stdout} stderr: {stderr}");
    assert!(
        stdout.contains("\"lineCoveragePercent\": 100"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("\"branchCoveragePercent\": 100"),
        "stdout: {stdout}"
    );
}

#[test]
fn the_rhai_gate_reports_a_syntax_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("broken.rhai"), "fn one() { let = ; }\n").expect("script write");
    let (code, stdout, stderr) = main_binary(&[
        "rhai-check",
        "--src",
        dir.path().to_string_lossy().as_ref(),
        "--output",
        "json",
    ]);
    assert_eq!(code, 1, "stdout: {stdout} stderr: {stderr}");
    assert!(
        stdout.contains("\"rule\": \"rhai/syntax\""),
        "stdout: {stdout}"
    );
}

#[test]
fn the_rhai_group_prints_its_help() {
    let (code, stdout, stderr) = main_binary(&["rhai", "check", "--help"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("line/branch coverage"), "{stdout}");

    // The group's own binary lists the group's commands instead of running
    // one, so it is the group's index rather than the command's usage.
    let (binary_code, binary_stdout, binary_stderr) =
        run(env!("CARGO_BIN_EXE_ole-utils-rhai"), &["--help"]);
    assert_eq!(binary_code, 0, "stderr: {binary_stderr}");
    assert!(binary_stdout.contains("check"), "{binary_stdout}");
}
