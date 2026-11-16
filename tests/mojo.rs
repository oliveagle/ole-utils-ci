// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `mojo` group: the gates that read the sources themselves.
//!
//! `unsafe` is one of them — it reads what the language calls unsafe and needs no toolchain — and
//! so is the check that a toolchain gate names the tool it could not find instead of passing.
//! The gates that drive the `mojo` binary are in `mojo_toolchain.rs`, against a fake `mojo` that
//! records the command line it was handed.

mod support;

use support::{main_binary, mojo_fixture, run};

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
