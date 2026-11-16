// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `mojo` group.
//!
//! The toolchain gates shell out to `mojo`, so what they are pinned on here is
//! the missing-tool complaint; `unsafe` reads the sources itself, and reads
//! both extensions the language gives a file.

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

/// Builds a throwaway project holding one Mojo source.
fn mojo_fixture(source: &str, name: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join(name), source).expect("source write");
    dir
}

#[test]
fn the_mojo_group_rejects_an_escape_hatch() {
    let dir = mojo_fixture("fn main():\n    let p = unsafe_ptr[x](1)\n", "main.mojo");
    let root = dir.path().to_string_lossy().into_owned();
    let (code, stdout, _) = main_binary(&["mojo", "unsafe", "--project-root", &root]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("unsafe_ptr"), "stdout: {stdout}");
}

#[test]
fn the_mojo_emoji_extension_is_a_source_too() {
    let dir = mojo_fixture("fn main():\n    let p = unsafe_load(0)\n", "main.🔥");
    let root = dir.path().to_string_lossy().into_owned();
    let (code, stdout, _) = main_binary(&["mojo", "unsafe", "--project-root", &root]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("unsafe_load"), "stdout: {stdout}");
}

#[test]
fn a_mojo_toolchain_gate_says_what_is_missing() {
    let dir = mojo_fixture("fn main():\n    print(1)\n", "main.mojo");
    let root = dir.path().to_string_lossy().into_owned();
    let (code, _, stderr) = main_binary(&[
        "mojo",
        "format",
        "--project-root",
        &root,
        "--tool",
        "/nonexistent/definitely-not-mojo",
    ]);
    assert_eq!(code, 1);
    assert!(stderr.contains("not found on PATH"), "stderr: {stderr}");
}

#[test]
fn the_mojo_binary_prints_the_same_help_as_the_group() {
    let (group_code, group_stdout, _) = main_binary(&["mojo", "--help"]);
    let (binary_code, binary_stdout, _) = run(env!("CARGO_BIN_EXE_ole-utils-mojo"), &["--help"]);
    assert_eq!(group_code, binary_code);
    assert_eq!(group_stdout, binary_stdout);
}
