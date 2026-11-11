// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the command runner.

use super::*;

#[test]
fn detects_missing_commands() {
    assert!(!Runner::command_exists("definitely-not-a-real-command-xyz"));
    assert!(Runner::command_exists("sh"));
}

#[test]
fn captures_output() {
    let r = Runner::new(".", false);
    let out = r.run("sh", &["-c", "echo hi"]).unwrap();
    assert!(out.contains("hi"));
}

#[test]
fn reports_failure_with_output() {
    let r = Runner::new(".", false);
    let err = r
        .run_checked("sh", &["-c", "echo boom >&2; exit 3"])
        .unwrap_err();
    assert!(err.contains("boom"));
    assert!(err.contains("exit code 3"));
}

#[test]
fn passes_extra_environment_to_the_child() {
    let r = Runner::new(".", false).with_env("DEVINE_UTILS_CI_PROBE", "yes");
    let out = r
        .run("sh", &["-c", "printf '%s' \"$DEVINE_UTILS_CI_PROBE\""])
        .unwrap();
    assert_eq!(out, "yes");
}

#[test]
fn pipes_producer_output_into_the_consumer() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.txt");
    let r = Runner::new(".", false);
    r.run_pipe(&["sh", "-c", "printf abc; printf def >&2"], &["cat"], &out)
        .unwrap();
    let got = std::fs::read_to_string(&out).unwrap();
    let mut chars: Vec<char> = got.chars().collect();
    chars.sort_unstable();
    assert_eq!(chars, vec!['a', 'b', 'c', 'd', 'e', 'f'], "{got}");
}

#[test]
fn reports_consumer_failures() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.txt");
    let r = Runner::new(".", false);
    let err = r
        .run_pipe(
            &["sh", "-c", "printf abc"],
            &["sh", "-c", "cat >/dev/null; exit 4"],
            &out,
        )
        .unwrap_err();
    assert!(err.contains("exit status 4"), "{err}");
}

#[test]
fn interactive_failures_are_worded_the_way_go_words_them() {
    let r = Runner::new(".", false);
    let err = r.run_interactive("sh", &["-c", "exit 3"]).unwrap_err();
    assert_eq!(err, "exit status 3");
}

#[test]
fn a_missing_interactive_program_reports_the_exec_error() {
    let r = Runner::new(".", false);
    let err = r
        .run_interactive("definitely-not-a-real-command-xyz", &[])
        .unwrap_err();
    assert_eq!(
        err,
        "exec: \"definitely-not-a-real-command-xyz\": executable file not found in $PATH"
    );
}
