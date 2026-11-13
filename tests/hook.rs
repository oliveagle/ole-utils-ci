// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `hook` command.
//!
//! Every test builds a throwaway repository and drives the compiled binary as
//! git would: the staged index for `pre-commit`, a message file for
//! `commit-msg`, and a `pre-push` line on standard input for `pre-push`. The
//! last test installs the hooks and lets `git commit` / `git push` run them,
//! which is the only evidence that the shims are wired correctly.

use std::path::Path;
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// A throwaway repository with an identity and one commit.
struct Repo {
    dir: TempDir,
}

impl Repo {
    fn new() -> Self {
        let repo = Repo {
            dir: tempfile::tempdir().expect("temp dir"),
        };
        repo.git(&["init", "-q", "-b", "main"]);
        repo.git(&["config", "user.email", "hook@test.invalid"]);
        repo.git(&["config", "user.name", "hook test"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo.write("base.txt", "base\n");
        repo.git(&["add", "."]);
        repo.git(&["commit", "-q", "-m", "feat: base"]);
        repo
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, name: &str, content: &str) {
        let path = self.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent dir");
        }
        std::fs::write(path, content).expect("fixture write");
    }

    /// Runs git in the repository and returns the exit code and stdout.
    fn git(&self, args: &[&str]) -> (i32, String) {
        let out = Command::new("git")
            .current_dir(self.path())
            .args(args)
            .output()
            .expect("failed to spawn git");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned()
                + &String::from_utf8_lossy(&out.stderr),
        )
    }

    /// Stages everything and returns the exit code and stdout of `git add`.
    fn stage_all(&self) -> (i32, String) {
        self.git(&["add", "-A"])
    }
}

/// Runs the compiled binary in `dir` and returns code, stdout and stderr.
fn run_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_devine-utils-ci"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("failed to spawn devine-utils-ci");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

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
