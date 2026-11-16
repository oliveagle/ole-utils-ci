// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `moonbit` group.
//!
//! The gate drives the MoonBit toolchain, so the test returns early when
//! `moon` is not installed: the suite runs everywhere and verifies the real
//! tool wherever it is present.

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

/// Whether the MoonBit toolchain is installed.
fn moonbit_toolchain_ready() -> bool {
    Command::new("moon")
        .arg("version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[test]
fn the_moonbit_gate_runs_tests_and_line_coverage() {
    if !moonbit_toolchain_ready() {
        return;
    }
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("moon.mod"),
        "name = \"demo-user/fixture\"\nversion = \"0.1.0\"\n",
    )
    .expect("module write");
    std::fs::write(dir.path().join("moon.pkg"), "").expect("package write");
    std::fs::write(
        dir.path().join("lib.mbt"),
        "pub fn classify(x: Int) -> Int {\n  if x > 0 {\n    1\n  } else {\n    0\n  }\n}\n",
    )
    .expect("source write");
    std::fs::write(
        dir.path().join("lib_test.mbt"),
        "test \"positive\" {\n  assert_eq(@fixture.classify(2), 1)\n}\n\ntest \"zero\" {\n  assert_eq(@fixture.classify(0), 0)\n}\n",
    )
    .expect("test write");
    let (code, stdout, stderr) = main_binary(&[
        "moonbit-check",
        "--src",
        dir.path().to_string_lossy().as_ref(),
        "--check-format=false",
        "--output",
        "json",
    ]);
    assert_eq!(code, 0, "stdout: {stdout} stderr: {stderr}");
    assert!(stdout.contains("\"testsRun\": 2"), "stdout: {stdout}");
    assert!(
        stdout.contains("\"lineCoveragePercent\": 100"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("\"branchCoverageSupported\": false"),
        "stdout: {stdout}"
    );
}

#[test]
fn the_moonbit_group_prints_its_help() {
    let (code, stdout, stderr) = main_binary(&["moonbit", "check", "--help"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("line coverage"), "{stdout}");

    // The group's own binary lists the group's commands instead of running
    // one, so it is the group's index rather than the command's usage.
    let (binary_code, binary_stdout, binary_stderr) =
        run(env!("CARGO_BIN_EXE_ole-utils-moonbit"), &["--help"]);
    assert_eq!(binary_code, 0, "stderr: {binary_stderr}");
    assert!(binary_stdout.contains("check"), "{binary_stdout}");
}
