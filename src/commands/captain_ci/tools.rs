// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `gocov*` / `go-junit-report` bootstrap.
//!
//! The template's `before_script` installs four tools at pinned versions; the
//! command does the same so a developer's artifacts match CI's. Nothing is
//! silently substituted: `go tool cover` would produce different files, which
//! is the one thing this command exists to avoid.

use std::path::Path;

use crate::runner::Runner;

/// The toolchain the template pins, in its `before_script` order.
pub(super) const TOOLS: [(&str, &str); 4] = [
    ("gocov", "github.com/axw/gocov/gocov@v1.1.0"),
    ("gocov-xml", "github.com/AlekSi/gocov-xml@v1.1.0"),
    (
        "gocov-html",
        "github.com/matm/gocov-html/cmd/gocov-html@latest",
    ),
    (
        "go-junit-report",
        "github.com/jstemmer/go-junit-report@v1.0.0",
    ),
];

/// `ensureCaptainCITools`: make every pinned binary reachable, installing the
/// missing ones and adopting any `go install` already put in `GOBIN` or
/// `GOPATH/bin` before re-installing.
pub(super) fn ensure(runner: &Runner) -> Result<(), String> {
    for (bin, spec) in TOOLS {
        if Runner::command_exists(bin) {
            continue;
        }
        if adopt(bin, runner) {
            continue;
        }
        println!("  ↳ go install {spec}");
        runner
            .run_interactive("go", &["install", spec])
            .map_err(|e| format!("go install {spec}: {e}"))?;
        if !Runner::command_exists(bin) {
            if adopt(bin, runner) {
                continue;
            }
            return Err(format!(
                "go install {spec} succeeded but {bin} is not on PATH — check {} / GOBIN / GOPATH/bin",
                bin_file_name(bin)
            ));
        }
    }
    Ok(())
}

/// Puts the install directory on `PATH` when the binary is already there.
fn adopt(bin: &str, runner: &Runner) -> bool {
    let Some(dir) = install_bin_dir(runner) else {
        return false;
    };
    if !Path::new(&dir).join(bin_file_name(bin)).is_file() {
        return false;
    }
    prepend_path(&dir);
    true
}

/// The directory `go install` writes binaries to: `GOBIN`, else the first
/// `GOPATH` entry plus `/bin`.
fn install_bin_dir(runner: &Runner) -> Option<String> {
    if let Ok(output) = runner.run_checked("go", &["env", "GOBIN"]) {
        let dir = output.trim();
        if !dir.is_empty() {
            return Some(dir.to_string());
        }
    }
    let output = runner.run_checked("go", &["env", "GOPATH"]).ok()?;
    let gopath = output.trim();
    if gopath.is_empty() {
        return None;
    }
    let first = gopath.split(path_list_separator()).next().unwrap_or(gopath);
    Some(format!("{first}/bin"))
}

/// Puts `dir` at the front of `PATH` for this process and every child.
fn prepend_path(dir: &str) {
    let current = std::env::var("PATH").unwrap_or_default();
    if current.is_empty() {
        std::env::set_var("PATH", dir);
        return;
    }
    std::env::set_var("PATH", format!("{dir}{}{current}", path_list_separator()));
}

/// The separator between `PATH` / `GOPATH` entries.
fn path_list_separator() -> char {
    if cfg!(windows) {
        ';'
    } else {
        ':'
    }
}

/// `go install` writes `.exe` on Windows; a guessed path has to match.
pub(super) fn bin_file_name(bin: &str) -> String {
    if cfg!(windows) && !bin.to_ascii_lowercase().ends_with(".exe") {
        return format!("{bin}.exe");
    }
    bin.to_string()
}
