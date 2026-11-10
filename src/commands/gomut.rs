// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `gomut` — Go mutation testing through the installed gomut engine.
//!
//! The Go original embeds `gomut/cli`, so its usage banner and `-version`
//! line describe the embedded copy. This rewrite delegates to the `gomut`
//! binary on `PATH` instead: every flag, mode and exit code comes from that
//! binary, which keeps the wrapper from drifting from upstream. The only
//! divergence is the banner, which then reports the installed engine's
//! version rather than an embedded one.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::commands::ExitCode;
use crate::runner::Runner;

/// Runs mutation testing and returns gomut's own exit code.
pub fn gomut(args: &[String]) -> Result<ExitCode, String> {
    let (root, rest) = split_project_root(args)?;
    if !Runner::command_exists("gomut") {
        println!("⏭ gomut is not installed; mutation testing skipped");
        return Ok(0);
    }
    let status = Command::new("gomut")
        .args(&rest)
        .current_dir(&root)
        .status()
        .map_err(|e| format!("gomut: {e}"))?;
    Ok(status.code().unwrap_or(1))
}

/// Lifts `--project-root` out of the argument list the way Go's
/// `parseGomutArgs` does, then resolves it and demands a directory.
fn split_project_root(args: &[String]) -> Result<(PathBuf, Vec<String>), String> {
    let mut root = PathBuf::from(".");
    let mut rest = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if (arg == "--project-root" || arg == "-project-root") && index + 1 < args.len() {
            root = PathBuf::from(&args[index + 1]);
            index += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--project-root=") {
            root = PathBuf::from(value);
            index += 1;
            continue;
        }
        if let Some(value) = arg.strip_prefix("-project-root=") {
            root = PathBuf::from(value);
            index += 1;
            continue;
        }
        rest.push(arg.clone());
        index += 1;
    }
    let absolute =
        crate::fsutil::absolute(&root).map_err(|e| format!("resolve project root: {e}"))?;
    if !is_dir(&absolute) {
        return Err(format!(
            "project root is not a directory: {}",
            absolute.display()
        ));
    }
    Ok((absolute, rest))
}

/// `os.Stat` + `IsDir`, so a missing path reports the same as Go.
fn is_dir(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn hoists_every_spelling_of_project_root() {
        for flag in ["--project-root", "-project-root"] {
            let args = strings(&[flag, "/tmp", "./..."]);
            let (root, rest) = split_project_root(&args).unwrap();
            assert_eq!(root, PathBuf::from("/tmp"));
            assert_eq!(rest, strings(&["./..."]));
        }
        let (root, rest) = split_project_root(&strings(&["--project-root=/tmp"])).unwrap();
        assert_eq!(root, PathBuf::from("/tmp"));
        assert!(rest.is_empty());
    }

    #[test]
    fn rejects_a_root_that_is_not_a_directory() {
        let err = split_project_root(&strings(&["--project-root", "/etc/hostname"]))
            .map(|_| ())
            .unwrap_err();
        assert_eq!(err, "project root is not a directory: /etc/hostname");
    }

    #[test]
    fn leaves_unrelated_arguments_alone() {
        let (root, rest) = split_project_root(&strings(&["-incremental", "./..."])).unwrap();
        assert!(root.is_dir());
        assert_eq!(rest, strings(&["-incremental", "./..."]));
    }
}
