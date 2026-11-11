// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `devine-utils-ci` binary.
//!
//! Every test drives the compiled executable as a black box and asserts on
//! the three things callers actually consume: the exit code, the human text
//! and the JSON payload.

use std::process::Command;

use tempfile::TempDir;

/// A source file that carries the licence header the `license` gate wants.
const LICENSED_SOURCE: &str = "// Copyright 2026 devine_utils contributors.\n\
                               // Licensed under the Apache License, Version 2.0.\n\
                               // SPDX-License-Identifier: Apache-2.0\n\
                               \n\
                               pub fn ok() {}\n";

/// The first marker the `bugs` gate looks for.
///
/// The literal never appears in this file because the repository's own
/// `bugs` gate scans `tests/*.rs` as well.
fn marker() -> &'static str {
    devine_utils_ci::gates::bugs::MARKERS[0]
}

/// Runs the compiled binary with `args` and returns its exit code, stdout and
/// stderr.
fn run(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_devine-utils-ci"))
        .args(args)
        .output()
        .expect("failed to spawn devine-utils-ci");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Builds a throwaway repository holding one Rust source file.
fn fixture(source: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("lib.rs"), source).expect("fixture write");
    dir
}

/// Runs `command` against the fixture in `dir`, appending `extra` flags.
fn run_at(command: &str, dir: &TempDir, extra: &[&str]) -> (i32, String, String) {
    let root = dir.path().to_string_lossy().into_owned();
    let mut args = vec![command, "--project-root", root.as_str()];
    args.extend_from_slice(extra);
    run(&args)
}

#[test]
fn clean_source_passes_the_bugs_gate() {
    let dir = fixture("pub fn clean() {}\n");
    let (code, stdout, _) = run_at("bugs", &dir, &[]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("no bug markers"), "stdout: {stdout}");
}

#[test]
fn marker_in_source_fails_the_bugs_gate() {
    let dir = fixture(&format!("const NOTE: &str = \"{}\";\n", marker()));
    let (code, stdout, _) = run_at("bugs", &dir, &[]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("MARKER VIOLATION"), "stdout: {stdout}");
}

#[test]
fn json_output_reports_a_passing_gate() {
    let dir = fixture("pub fn clean() {}\n");
    let (code, stdout, _) = run_at("bugs", &dir, &["--output", "json"]);
    assert_eq!(code, 0, "stdout: {stdout}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["command"], "bugs");
    assert_eq!(value["ok"], true);
    assert!(
        value.get("diagnostics").is_none(),
        "an empty diagnostic list is omitted: {stdout}"
    );
}

#[test]
fn json_output_reports_a_failing_gate() {
    let dir = fixture(&format!("const NOTE: &str = \"{}\";\n", marker()));
    let (code, stdout, _) = run_at("bugs", &dir, &["--output", "json"]);
    assert_eq!(code, 1, "stdout: {stdout}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["command"], "bugs");
    assert_eq!(value["ok"], false);
    let diagnostics = value["diagnostics"].as_array().expect("diagnostics array");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], "error");
    assert_eq!(diagnostics[0]["rule"], marker());
}

#[test]
fn license_gate_rejects_a_headerless_source_file() {
    let dir = fixture("pub fn bare() {}\n");
    let (code, stdout, _) = run_at("license", &dir, &[]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("lib.rs"), "stdout: {stdout}");
    assert!(
        stdout.contains("without license header"),
        "stdout: {stdout}"
    );
}

#[test]
fn license_gate_accepts_a_headered_source_file() {
    let dir = fixture(LICENSED_SOURCE);
    let (code, stdout, _) = run_at("license", &dir, &[]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("License check passed"), "stdout: {stdout}");
}

#[test]
fn unknown_command_is_rejected() {
    // Go prints the complaint and the usage block on stdout, then exits 1.
    let (code, stdout, stderr) = run(&["definitely-not-a-command"]);
    assert_eq!(code, 1);
    assert!(
        stdout.starts_with("Unknown command: definitely-not-a-command\n\n"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("devine-utils-ci <command> [flags]"),
        "stdout: {stdout}"
    );
    assert!(stderr.is_empty(), "stderr: {stderr}");
}

#[test]
fn a_dashed_first_argument_is_still_a_command() {
    // Go takes os.Args[1] verbatim, so --version and -h are commands.
    let (code, stdout, _) = run(&["--version"]);
    assert_eq!(code, 0);
    assert!(
        stdout.starts_with("devine-utils-ci version"),
        "stdout: {stdout}"
    );

    let (code, stdout, _) = run(&["-h"]);
    assert_eq!(code, 0);
    assert!(
        stdout.starts_with("devine-utils-ci — Unified CI Quality Gate Tool"),
        "stdout: {stdout}"
    );
}

#[test]
fn the_usage_block_keeps_the_go_text() {
    let (code, stdout, _) = run(&["help"]);
    assert_eq!(code, 0);
    // Every line of the Go usage block must still be there, in order.
    for needle in [
        "Commands:",
        "  pipeline-watch       Interactive TUI dashboard",
        "Global Flags:",
        "Output modes (per-subcommand; see AGENTS.md §9):",
        "Key bindings (TUI):",
    ] {
        assert!(stdout.contains(needle), "missing {needle:?} in: {stdout}");
    }
    // The Rust-only gates are documented after the Go block.
    assert!(
        stdout.contains("Rust gates (added by this build"),
        "stdout: {stdout}"
    );
}

#[test]
fn version_prints_the_banner() {
    let (code, stdout, _) = run(&["version"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("devine-utils-ci version"),
        "stdout: {stdout}"
    );
}
