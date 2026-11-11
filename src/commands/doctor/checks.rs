// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The checks `doctor` runs, and the process helpers every check shares.
//!
//! The Go original keeps every probe in one `internal/doctor.go`; the
//! file-length budget here is tighter than that file, so the probes live in
//! [`super::toolchain`], [`super::metadata`] and [`super::project`] while
//! this module holds the result type and the four helpers they all need.

use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

use crate::commands::ci_recipe_util::go_errno;
use crate::httpd::{self, Response};

/// One line of the report: a name, a status and the words around it.
#[derive(Debug, Clone)]
pub(super) struct Check {
    /// The label; the indented checks carry their own leading spaces.
    pub name: String,
    /// `pass`, `warn` or `fail`, spelled the way the Go original spells them.
    pub status: &'static str,
    /// What was observed, empty when the name already says it.
    pub detail: String,
    /// The advice printed after the light bulb, empty when there is none.
    pub hint: String,
}

impl Check {
    /// A check that found nothing wrong.
    pub(super) fn pass(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status: "pass",
            detail: detail.into(),
            hint: String::new(),
        }
    }

    /// A check worth mentioning without failing the command.
    pub(super) fn warn(name: &str, detail: impl Into<String>, hint: &str) -> Self {
        Self {
            name: name.to_string(),
            status: "warn",
            detail: detail.into(),
            hint: hint.to_string(),
        }
    }

    /// A check that fails the command once the report has been printed.
    pub(super) fn fail(name: &str, detail: impl Into<String>, hint: &str) -> Self {
        Self {
            name: name.to_string(),
            status: "fail",
            detail: detail.into(),
            hint: hint.to_string(),
        }
    }

    /// Whether the command should leave with `1`.
    pub(super) fn failed(&self) -> bool {
        self.status == "fail"
    }
}

/// Every check, in the order the Go original appends them.
pub(super) fn run(root: &Path) -> Vec<Check> {
    let mut results = super::toolchain::go_version(root);
    results.extend(super::toolchain::go_proxy());
    results.extend(super::toolchain::go_sum_env());
    results.extend(super::metadata::metadata_domain());
    results.extend(super::project::run(root));
    results
}

/// One `GET`, framed the way `net/http` frames a transport failure:
/// `Get "<url>": <cause>`. The frame is Go's; the cause behind it is
/// `curl`'s own wording, which is the third-party error text the
/// CHANGELOG records as a known divergence.
pub(super) fn get(url: &str, timeout: Duration) -> Result<Response, String> {
    httpd::request("GET", url, &[], None, timeout).map_err(|error| format!("Get {url:?}: {error}"))
}

/// Runs `go <args>` in `root` with the lookup, the directory probe and the
/// combined output `exec.Cmd.CombinedOutput` applies.
pub(super) fn go(root: &Path, args: &[&str]) -> Result<Output, String> {
    let binary = look_path()?;
    // `os.startProcess` stats the working directory first and renames that
    // failure to `chdir`, so a missing root is reported before the child
    // can be started at all.
    if let Err(error) = std::fs::metadata(root) {
        return Err(format!("chdir {}: {}", root.display(), go_errno(&error)));
    }
    match Command::new(&binary).current_dir(root).args(args).output() {
        Ok(output) => Ok(output),
        Err(error) => Err(format!("fork/exec {binary}: {}", go_errno(&error))),
    }
}

/// Go's `exec.LookPath("go")`: the first `PATH` entry that carries an
/// executable, or the error `exec.Command` records when no entry does.
fn look_path() -> Result<String, String> {
    let not_found = "exec: \"go\": executable file not found in $PATH".to_string();
    let Some(paths) = std::env::var_os("PATH") else {
        return Err(not_found);
    };
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join("go");
        if is_executable(&candidate) {
            return Ok(candidate.to_string_lossy().into_owned());
        }
    }
    Err(not_found)
}

/// Go's `findExecutable`: stat-able, not a directory, one exec bit set.
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(metadata) => !metadata.is_dir() && metadata.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

/// The two streams of one `Output`, stdout first: what `CombinedOutput`
/// hands back whenever only one of them carries text.
pub(super) fn combined(output: &Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}
