// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `pre-push` hook.
//!
//! A push check reads the ref lines git pipes into the hook on standard
//! input, so these tests drive the binary the way git does: the same three
//! arguments, the same two lines on standard input.

mod support;

use std::process::{Command, Stdio};

use support::Repo;

/// Runs `hook pre-push` with the ref lines git would pipe in.
fn push(repo: &Repo, refs: &str) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_devine-utils-ci"))
        .current_dir(repo.path())
        .args(["hook", "pre-push", "origin", "git@example.invalid:o/r.git"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn the hook");
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().expect("stdin");
        stdin
            .write_all(refs.as_bytes())
            .expect("failed to write the refs");
    }
    let out = child.wait_with_output().expect("wait");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn a_push_to_main_is_refused() {
    let repo = Repo::new();
    let (code, stdout, stderr) = push(
        &repo,
        "refs/heads/main 1111111111111111111111111111111111111111 refs/heads/main 1111111111111111111111111111111111111111\n",
    );
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("main is protected"), "{stdout}");
    assert!(stderr.contains("open a merge request"), "{stderr}");
}

#[test]
fn a_push_from_a_stray_branch_name_is_refused() {
    let repo = Repo::new();
    let (code, stdout, _) = push(
        &repo,
        "refs/heads/my-work 1111111111111111111111111111111111111111 refs/heads/my-work 0000000000000000000000000000000000000000\n",
    );
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("is not a <type>/<slug> branch"), "{stdout}");
}

#[test]
fn a_push_from_a_typed_branch_passes() {
    let repo = Repo::new();
    let (code, stdout, _) = push(
        &repo,
        "refs/heads/feat/hook-checks 1111111111111111111111111111111111111111 refs/heads/feat/hook-checks 0000000000000000000000000000000000000000\nrefs/tags/v1.0.0 1111111111111111111111111111111111111111 refs/tags/v1.0.0 0000000000000000000000000000000000000000\n",
    );
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(
        stdout.contains("branch feat/hook-checks may be pushed"),
        "{stdout}"
    );
    assert!(stdout.contains("tag v1.0.0 may be pushed"), "{stdout}");
}
