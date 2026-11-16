// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The throwaway repository the hook tests drive.
//!
//! Every hook test needs a git repository with an identity and one commit,
//! and every hook test needs to run the compiled binary inside it. Both live
//! here so the test files that need them do not repeat them.
//!
//! Every integration test binary compiles this module on its own, so a
//! helper that only one of them uses is dead code in the others. That is
//! expected here, and the lint is silenced for this module alone.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// A throwaway repository with an identity and one commit.
pub struct Repo {
    dir: TempDir,
}

impl Repo {
    pub fn new() -> Self {
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

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn write(&self, name: &str, content: &str) {
        let path = self.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent dir");
        }
        std::fs::write(path, content).expect("fixture write");
    }

    /// Runs git in the repository and returns the exit code and stdout.
    pub fn git(&self, args: &[&str]) -> (i32, String) {
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
    pub fn stage_all(&self) -> (i32, String) {
        self.git(&["add", "-A"])
    }
}

/// Runs the compiled binary in `dir` and returns code, stdout and stderr.
pub fn run_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
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

// -------------------------------------------------------------------------------------------
// The harness the language-group tests share: a way to run the compiled binary, with or
// without an environment, and a fake language tool that records what it was asked to do.
// -------------------------------------------------------------------------------------------

/// Runs a binary with `args` and returns its exit code, stdout and stderr.
pub fn run(binary: &str, args: &[&str]) -> (i32, String, String) {
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
pub fn main_binary(args: &[&str]) -> (i32, String, String) {
    run(env!("CARGO_BIN_EXE_devine-utils-ci"), args)
}

/// Runs the main binary with extra environment, which is how the fake tool is told what to do.
pub fn main_binary_env(args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_devine-utils-ci"));
    command.args(args);
    for (key, value) in env {
        command.env(key, value);
    }
    let output = command.output().expect("failed to spawn the binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A stand-in for the `mojo` binary: records its arguments, and can rewrite one file it was given
/// so the format gate has something to detect.
///
/// The rewrite is what a real `mojo format` does — it edits in place. That the gate then has to
/// notice without letting the edit reach the project is the whole point of the scratch copy.
#[cfg(unix)]
pub fn fake_mojo(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("fake-mojo.sh");
    std::fs::write(
        &script,
        r#"#!/bin/sh
echo "$*" >> "$FAKE_ARGS"
if [ -n "$FAKE_REFORMAT" ]; then
  for arg in "$@"; do
    case "$arg" in
      */"$FAKE_REFORMAT") printf 'def changed():
    pass
' > "$arg" ;;
    esac
  done
fi
exit "${FAKE_EXIT:-0}"
"#,
    )
    .expect("fake tool write");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    script
}

/// Every command line the fake tool was handed, one per invocation.
#[cfg(unix)]
pub fn invocations(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// Builds a throwaway project holding one Mojo source.
pub fn mojo_fixture(source: &str, name: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join(name), source).expect("source write");
    dir
}
