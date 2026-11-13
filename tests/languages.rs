// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the language-scoped subcommands.
//!
//! Every language group is reachable twice — as a subcommand of the main
//! binary and as a standalone binary — and both spellings must behave the
//! same way.

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

/// Builds a throwaway repository holding one clean source file.
fn fixture() -> TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("lib.rs"), "pub fn clean() {}\n").expect("fixture write");
    dir
}

#[test]
fn a_group_lists_its_own_commands() {
    let (code, stdout, stderr) = main_binary(&["rust", "--help"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stdout.contains("the Rust gates of devine-utils-ci"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("clippy"), "stdout: {stdout}");
    assert!(stdout.contains("msrv"), "stdout: {stdout}");
}

#[test]
fn every_group_has_a_help_block() {
    for group in ["golang", "rust", "tengo", "proto", "web", "common"] {
        let (code, stdout, stderr) = main_binary(&[group, "--help"]);
        assert_eq!(code, 0, "{group}: stderr: {stderr}");
        assert!(
            stdout.contains(&format!("devine-utils-ci {group} <command>")),
            "{group}: stdout: {stdout}"
        );
    }
}

#[test]
fn a_language_binary_prints_the_same_help_as_the_group() {
    let (group_code, group_stdout, _) = main_binary(&["tengo", "--help"]);
    let (binary_code, binary_stdout, _) = run(env!("CARGO_BIN_EXE_ole-utils-tengo"), &["--help"]);
    assert_eq!(group_code, binary_code);
    assert_eq!(group_stdout, binary_stdout);
}

#[test]
fn a_language_binary_names_itself_in_the_version() {
    let (code, stdout, stderr) = run(env!("CARGO_BIN_EXE_ole-utils-golang"), &["version"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stdout.starts_with("ole-utils-golang version "),
        "stdout: {stdout}"
    );
}

#[test]
fn an_unknown_group_command_exits_one() {
    let (code, stdout, stderr) = main_binary(&["rust", "nope"]);
    assert_eq!(code, 1);
    assert!(
        stdout.contains("Unknown rust command: nope"),
        "stdout: {stdout}"
    );
    assert!(stderr.is_empty(), "stderr: {stderr}");
}

#[test]
fn a_group_forwards_its_flags_to_the_command() {
    let dir = fixture();
    let root = dir.path().to_string_lossy().into_owned();
    let (code, stdout, stderr) = main_binary(&["common", "bugs", "--project-root", &root]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("no bug markers"), "stdout: {stdout}");
}

#[test]
fn a_language_binary_forwards_its_flags_to_the_command() {
    let dir = fixture();
    let root = dir.path().to_string_lossy().into_owned();
    let (code, stdout, stderr) = run(
        env!("CARGO_BIN_EXE_ole-utils-common"),
        &["bugs", "--project-root", &root],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("no bug markers"), "stdout: {stdout}");
}

#[test]
fn the_flat_commands_still_work() {
    // The group layer sits in front of the flat surface, so it must not
    // swallow the commands it does not own.
    let dir = fixture();
    let root = dir.path().to_string_lossy().into_owned();
    let (code, stdout, stderr) = main_binary(&["bugs", "--project-root", &root]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("no bug markers"), "stdout: {stdout}");
}

#[test]
fn the_alias_binary_runs_the_whole_tool() {
    let (code, stdout, stderr) = run(env!("CARGO_BIN_EXE_ole-utils-ci"), &["version"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stdout.starts_with("ole-utils-ci version "),
        "stdout: {stdout}"
    );
}
