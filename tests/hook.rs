// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `hook` command.
//!
//! Every test builds a throwaway repository and drives the compiled binary as
//! git would: the staged index for `pre-commit`, a message file for
//! `commit-msg`, and the installed shims for `git commit` / `git push`. The
//! push checks live in `hook_push.rs`, because they feed ref lines on
//! standard input and need their own harness.

mod support;

use std::process::Command;

use support::{run_in, Repo};

#[test]
fn the_command_lists_its_subcommands_and_runs_nothing() {
    let repo = Repo::new();
    let (code, stdout, stderr) = run_in(repo.path(), &["hook"]);
    assert_eq!(code, 1, "stderr: {stderr}");
    assert!(
        stdout.contains("hook — the commit and push policy checks"),
        "{stdout}"
    );
    assert!(stdout.contains("pre-commit"), "{stdout}");
    assert!(
        stdout.contains(&format!("default {}", 20)),
        "the 20-file default must be visible: {stdout}"
    );
    assert!(
        !stdout.contains("staged files"),
        "nothing may run: {stdout}"
    );
}

#[test]
fn the_command_has_a_help_block() {
    let repo = Repo::new();
    let (code, stdout, _) = run_in(repo.path(), &["hook", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("devine-utils-ci hook <command> [flags]"),
        "{stdout}"
    );
}

#[test]
fn a_commit_over_the_file_budget_is_refused() {
    let repo = Repo::new();
    for index in 0..21 {
        repo.write(&format!("src/f{index}.txt"), "x\n");
    }
    repo.stage_all();
    let (code, stdout, stderr) = run_in(repo.path(), &["hook", "pre-commit"]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("over the 20-file budget"), "{stdout}");
    assert!(
        stderr.contains("split this into commits that each build"),
        "{stderr}"
    );
}

#[test]
fn a_commit_at_the_file_budget_passes() {
    let repo = Repo::new();
    for index in 0..20 {
        repo.write(&format!("src/f{index}.txt"), "x\n");
    }
    repo.stage_all();
    let (code, stdout, _) = run_in(repo.path(), &["hook", "pre-commit"]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("within the 20-file budget"), "{stdout}");
}

#[test]
fn the_file_budget_can_be_tightened_for_one_run() {
    let repo = Repo::new();
    for index in 0..4 {
        repo.write(&format!("src/f{index}.txt"), "x\n");
    }
    repo.stage_all();
    let (code, stdout, _) = run_in(repo.path(), &["hook", "pre-commit", "--max-files", "3"]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("over the 3-file budget"), "{stdout}");
}

#[test]
fn a_large_staged_file_is_refused() {
    let repo = Repo::new();
    repo.write("big.bin", &"x".repeat(4096));
    repo.stage_all();
    let (code, stdout, _) = run_in(
        repo.path(),
        &["hook", "pre-commit", "--max-file-bytes", "1k"],
    );
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("big.bin is 4.0 KiB"), "{stdout}");
}

#[test]
fn a_credential_looking_file_is_refused_unless_allowed() {
    let repo = Repo::new();
    repo.write("deploy/.env", "TOKEN=abc\n");
    repo.stage_all();
    let (code, stdout, _) = run_in(repo.path(), &["hook", "pre-commit"]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("looks like a credential"), "{stdout}");

    let (code, stdout, _) = run_in(repo.path(), &["hook", "pre-commit", "--allow-secret-paths"]);
    assert_eq!(code, 0, "stdout: {stdout}");
}

#[test]
fn a_staged_conflict_marker_is_refused() {
    let repo = Repo::new();
    // Built at runtime: this file must not contain a marker of its own.
    let marker = format!("{} HEAD\n", "<".repeat(7));
    repo.write("conflict.txt", &format!("a\n{marker}mine\n"));
    repo.stage_all();
    let (code, stdout, stderr) = run_in(repo.path(), &["hook", "pre-commit"]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stderr.contains("conflict"), "stderr: {stderr}");
}

#[test]
fn a_conventional_message_passes_and_a_bare_word_does_not() {
    let repo = Repo::new();
    let good = repo.path().join("good-msg");
    std::fs::write(&good, "feat(hook): refuse oversized commits\n\n# comment\n").expect("write");
    let (code, stdout, _) = run_in(repo.path(), &["hook", "commit-msg", good.to_str().unwrap()]);
    assert_eq!(code, 0, "stdout: {stdout}");

    let bad = repo.path().join("bad-msg");
    std::fs::write(&bad, "wip\n").expect("write");
    let (code, stdout, stderr) =
        run_in(repo.path(), &["hook", "commit-msg", bad.to_str().unwrap()]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(
        stderr.contains("is not `<type>: <description>`"),
        "{stderr}"
    );
}

#[test]
fn install_status_and_uninstall_round_trip() {
    let repo = Repo::new();
    let (code, stdout, _) = run_in(repo.path(), &["hook", "install"]);
    assert_eq!(code, 0, "stdout: {stdout}");
    for name in ["pre-commit", "commit-msg", "pre-push"] {
        let path = repo.path().join(".git/hooks").join(name);
        let text = std::fs::read_to_string(&path).expect("hook written");
        assert!(text.contains("devine-utils-ci"), "{name}: {text}");
        assert!(text.contains(&format!("hook {name}")), "{name}: {text}");
    }
    let (code, stdout, _) = run_in(repo.path(), &["hook", "status"]);
    assert_eq!(code, 0);
    assert_eq!(
        stdout.matches("installed by devine-utils-ci").count(),
        3,
        "{stdout}"
    );

    let (code, _, _) = run_in(repo.path(), &["hook", "uninstall"]);
    assert_eq!(code, 0);
    let (_, stdout, _) = run_in(repo.path(), &["hook", "status"]);
    assert_eq!(stdout.matches("not installed").count(), 3, "{stdout}");
}

#[test]
fn a_foreign_hook_is_never_replaced_without_force() {
    let repo = Repo::new();
    let path = repo.path().join(".git/hooks/pre-push");
    std::fs::write(&path, "#!/bin/sh\necho somebody else\n").expect("write");
    let (code, stdout, stderr) = run_in(repo.path(), &["hook", "install"]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stderr.contains("belong to another tool"), "{stderr}");
    let text = std::fs::read_to_string(&path).expect("read");
    assert!(
        text.contains("somebody else"),
        "the foreign hook must survive"
    );
    // Nothing else was written either: the install is all or nothing.
    assert!(!repo.path().join(".git/hooks/pre-commit").exists());

    let (code, _, _) = run_in(repo.path(), &["hook", "install", "--force"]);
    assert_eq!(code, 0);
    let text = std::fs::read_to_string(&path).expect("read");
    assert!(text.contains("devine-utils-ci"), "{text}");
}

#[test]
fn the_installed_hooks_stop_git_itself() {
    let repo = Repo::new();
    let (code, _, _) = run_in(repo.path(), &["hook", "install"]);
    assert_eq!(code, 0);

    for index in 0..21 {
        repo.write(&format!("src/f{index}.txt"), "x\n");
    }
    repo.stage_all();
    let (code, output) = repo.git(&["commit", "-m", "feat: twenty one files at once"]);
    assert_eq!(code, 1, "{output}");
    assert!(output.contains("pre-commit"), "{output}");

    // Drop the rejected change so the next commit stages one file.
    repo.git(&["reset", "-q"]);
    std::fs::remove_dir_all(repo.path().join("src")).expect("drop the rejected files");
    repo.write("only.txt", "x\n");
    repo.stage_all();
    let (code, output) = repo.git(&["commit", "-m", "wip"]);
    assert_eq!(code, 1, "{output}");
    assert!(output.contains("commit-msg"), "{output}");

    let (code, output) = repo.git(&["commit", "-m", "feat: add only.txt"]);
    assert_eq!(code, 0, "{output}");
}

#[test]
fn the_installed_hooks_stop_a_push_to_main() {
    let repo = Repo::new();
    let (code, _, _) = run_in(repo.path(), &["hook", "install"]);
    assert_eq!(code, 0);
    let remote = tempfile::tempdir().expect("remote");
    let (code, _) = Command::new("git")
        .current_dir(remote.path())
        .args(["init", "-q", "--bare"])
        .output()
        .map(|o| (o.status.code().unwrap_or(-1), String::new()))
        .expect("bare remote");
    assert_eq!(code, 0);
    let url = remote.path().to_string_lossy().to_string();
    repo.git(&["remote", "add", "origin", &url]);

    let (code, output) = repo.git(&["push", "origin", "main"]);
    assert_eq!(code, 1, "{output}");
    assert!(output.contains("main is protected"), "{output}");

    repo.git(&["checkout", "-q", "-b", "feat/pushed"]);
    let (code, output) = repo.git(&["push", "origin", "feat/pushed"]);
    assert_eq!(code, 0, "{output}");
}

#[test]
fn the_common_group_reaches_the_hook_command() {
    let repo = Repo::new();
    let (code, stdout, _) = run_in(repo.path(), &["common", "hook", "--help"]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(
        stdout.contains("hook — the commit and push policy checks"),
        "{stdout}"
    );
}
