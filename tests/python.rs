// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `python` group.
//!
//! Two kinds of gate sit behind the group, and the difference is what these
//! tests pin: the ones that drive the project's own tools, which must name a
//! missing tool instead of quietly passing, and the ones that read the sources
//! and need nothing installed at all.

use std::process::Command;

use tempfile::TempDir;

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

/// Builds a throwaway project holding one Python source and one requirements
/// file.
fn py_fixture(source: &str, requirements: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )
    .expect("pyproject write");
    std::fs::write(dir.path().join("app.py"), source).expect("source write");
    std::fs::write(dir.path().join("requirements.txt"), requirements).expect("requirements write");
    dir
}

#[test]
fn the_python_group_rejects_a_call_that_starts_a_shell() {
    let dir = py_fixture("import os\n\nos.system('ls')\n", "requests==2.31.0\n");
    let root = dir.path().to_string_lossy().into_owned();
    let (code, stdout, _) = main_binary(&["python", "no-shell-exec", "--project-root", &root]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("os.system"), "stdout: {stdout}");
}

#[test]
fn the_python_group_rejects_an_unpinned_requirement() {
    let dir = py_fixture("x = 1\n", "requests==2.31.0\nflask>=3\n");
    let root = dir.path().to_string_lossy().into_owned();
    let (code, stdout, _) = main_binary(&["python", "deps", "--project-root", &root]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("flask"), "stdout: {stdout}");
}

#[test]
fn a_pinned_python_project_passes_the_source_gates() {
    let dir = py_fixture(
        "# Copyright 2026 fixture\nimport subprocess\n\n\ndef run(cmd):\n    subprocess.run([cmd])\n",
        "requests==2.31.0\n",
    );
    let root = dir.path().to_string_lossy().into_owned();
    for gate in ["no-shell-exec", "deps"] {
        let (code, stdout, stderr) = main_binary(&["python", gate, "--project-root", &root]);
        assert_eq!(code, 0, "{gate}: stdout: {stdout} stderr: {stderr}");
    }
}

#[test]
fn a_missing_python_tool_is_named_in_the_complaint() {
    let dir = py_fixture("x = 1\n", "requests==2.31.0\n");
    let root = dir.path().to_string_lossy().into_owned();
    let (code, stdout, stderr) = main_binary(&[
        "python",
        "lint",
        "--project-root",
        &root,
        "--tool",
        "/nonexistent/definitely-not-a-linter",
    ]);
    assert_eq!(code, 1);
    assert!(
        stderr.contains("not found on PATH"),
        "stderr: {stderr} stdout: {stdout}"
    );
    assert!(
        stderr.contains("definitely-not-a-linter"),
        "stderr: {stderr}"
    );
}

#[test]
fn the_python_binary_prints_the_same_help_as_the_group() {
    let (group_code, group_stdout, _) = main_binary(&["python", "--help"]);
    let (binary_code, binary_stdout, _) = run(env!("CARGO_BIN_EXE_ole-utils-python"), &["--help"]);
    assert_eq!(group_code, binary_code);
    assert_eq!(group_stdout, binary_stdout);
}

#[cfg(unix)]
#[test]
fn a_missing_coverage_plugin_is_named() {
    use std::os::unix::fs::PermissionsExt;

    // An interpreter that cannot import pytest_cov: the probe fails before
    // pytest could answer the `--cov` arguments with its usage banner.
    let dir = py_fixture("x = 1\n", "requests==2.31.0\n");
    let fake = dir.path().join("interpreter-without-the-plugin");
    std::fs::write(&fake, "#!/bin/sh\nexit 3\n").expect("fake interpreter write");
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let root = dir.path().to_string_lossy().into_owned();
    let interpreter = fake.to_string_lossy().into_owned();

    let (code, stdout, stderr) = main_binary(&[
        "python",
        "coverage",
        "--project-root",
        &root,
        "--interpreter",
        &interpreter,
    ]);
    assert_eq!(code, 1, "stdout: {stdout} stderr: {stderr}");
    assert!(stderr.contains("pytest_cov"), "stderr: {stderr}");
    assert!(
        stderr.contains("pip install pytest-cov"),
        "stderr: {stderr}"
    );
}
