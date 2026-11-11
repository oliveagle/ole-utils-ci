// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `web-lint` command: syntax sanity for the front-end sources.
//!
//! Go builds a private `flag.FlagSet` for this command and reports with the
//! `web_lint` result envelope, so its flags never reach the shared parser and
//! its exit codes are 0 / 1 / 2 rather than the usual 0 / 1. Both the flag
//! contract and the envelope are reproduced here; the scanners themselves are
//! the dependency-free ones in [`super::engine`].

use crate::cli::{report_parse_error, FlagSet, ParseError};
use crate::output;

use super::usage::FRIENDLY_USAGE;

mod render;
mod run;

pub use run::Failure;

/// Lints the configured paths and prints the text result.
///
/// The `lint` command owns the policy — the paths, the engine and the
/// `--fail-on` severity — so this is the entry point it uses. The complaint
/// is the one Go's `runConfiguredWebLint` returns.
pub fn lint_paths(options: &Options) -> Result<(), String> {
    match run::run(options) {
        Ok(_) => Ok(()),
        Err(run::Failure::Io(message)) => Err(message),
        Err(run::Failure::Usage(error)) => Err(format!("{} ({})", error.message, error.code)),
    }
}

/// The default per-run file cap.
const DEFAULT_MAX_FILES: i64 = 200;

/// The default per-file byte cap.
const DEFAULT_MAX_FILE_SIZE: i64 = 1_048_576;

/// One parsed `web-lint` command line.
#[derive(Debug)]
pub struct Options {
    /// Project root; also the default security root.
    pub project_root: String,
    /// File or directory to lint.
    pub path: String,
    /// Inline source to lint.
    pub content: String,
    /// Language of the inline source.
    pub language: String,
    /// Security root: resolved paths must stay inside.
    pub root: String,
    /// `auto`, `builtin` or `external`.
    pub ts_engine: String,
    /// tsconfig.json path for the external engine.
    pub tsconfig: String,
    /// Path substrings to skip.
    pub ignore: Vec<String>,
    /// Cap on files linted per run.
    pub max_files: i64,
    /// Per-file byte cap.
    pub max_file_size: i64,
    /// `text`, `json` or `toon`.
    pub format: String,
    /// Severity that makes the exit code 1.
    pub fail_on: String,
}

/// The private flag set, spelled the way Go's `webLint` builds it.
pub fn flag_set() -> FlagSet {
    let mut fs = FlagSet::new();
    fs.str_flag("project-root", ".")
        .str_flag("path", "")
        .str_flag("content", "")
        .str_flag("language", "")
        .str_flag("root", "")
        .str_flag("ts-engine", "auto")
        .str_flag("tsconfig", "")
        .int_flag("max-files", DEFAULT_MAX_FILES)
        .int_flag("max-file-size", DEFAULT_MAX_FILE_SIZE)
        .int_flag("external-timeout-sec", 60)
        .str_flag("format", "text")
        .str_flag("fail-on", "error")
        .value_flag("ignore")
        .describe(
            "project-root",
            "project root; also the default security root",
        )
        .describe(
            "path",
            "file or directory to lint (mutually exclusive with --content)",
        )
        .describe("content", "inline source to lint (requires --language)")
        .describe(
            "language",
            "html|css|js|ts|tsx|jsx (required with --content)",
        )
        .describe("root", "security root: resolved paths must stay inside")
        .describe("ts-engine", "auto|builtin|external")
        .describe("tsconfig", "tsconfig.json path for the external tsc engine")
        .describe("max-files", "cap on files linted per run")
        .describe("max-file-size", "per-file byte cap")
        .describe("external-timeout-sec", "timeout for the external tsc run")
        .describe(
            "ignore",
            "comma-separated path substrings to skip (repeatable)",
        )
        .describe("format", "text|json|toon")
        .describe(
            "fail-on",
            "error|warning|none — severity that makes exit code 1",
        );
    fs
}

impl Options {
    /// Reads the parsed flags back out of the set.
    pub fn from(fs: &FlagSet) -> Self {
        let text = |name: &str| {
            fs.get(name)
                .map(|value| value.as_str(""))
                .unwrap_or_default()
        };
        let number = |name: &str, fallback: i64| {
            fs.get(name)
                .map(|value| value.as_int(fallback))
                .unwrap_or(fallback)
        };
        Self {
            project_root: text("project-root"),
            path: text("path"),
            content: text("content"),
            language: text("language"),
            root: text("root"),
            ts_engine: text("ts-engine"),
            tsconfig: text("tsconfig"),
            ignore: fs
                .get("ignore")
                .map(|value| value.as_values().to_vec())
                .unwrap_or_default(),
            max_files: number("max-files", DEFAULT_MAX_FILES),
            max_file_size: number("max-file-size", DEFAULT_MAX_FILE_SIZE),
            format: text("format"),
            fail_on: text("fail-on"),
        }
    }
}

/// The `web-lint` command. `args` is everything after the subcommand name.
pub fn web_lint(args: &[String]) -> Result<i32, String> {
    let mut fs = flag_set();
    if let Err(error) = fs.parse(args) {
        report_parse_error(&error, &fs.render_usage("web-lint"));
        if matches!(error, ParseError::Help) {
            print!("{FRIENDLY_USAGE}");
            return Ok(0);
        }
        eprintln!("Error: {error}");
        return Ok(2);
    }
    let options = Options::from(&fs);
    if let Err(message) = output::validate_mode(&options.format) {
        eprintln!("web-lint: {message}");
        eprintln!("Error: {message}");
        return Ok(2);
    }
    match run::run(&options) {
        Ok(code) => Ok(code),
        Err(Failure::Usage(error)) => {
            eprintln!("web-lint: {} ({})", error.message, error.code);
            eprintln!("Error: {}", error.message);
            Ok(2)
        }
        Err(Failure::Io(message)) => Err(message),
    }
}

#[cfg(test)]
mod tests;
