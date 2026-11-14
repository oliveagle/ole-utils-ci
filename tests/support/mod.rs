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

use std::path::Path;
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
