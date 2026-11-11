// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The focused lint gates: a private flag set and TOON output by default.
//!
//! `go-pin`, `no-shell-exec`, `plugin-freshness` and `web-lint` do not share
//! the global flag set. The Go original builds a private `flag.FlagSet` for
//! each of them, defaults `--output` to `toon`, exits 2 when the command line
//! is refused and 1 when the gate finds something — the contract eslint and
//! tsc use. This module reproduces that contract so a gate can carry its own
//! flags without the global parser seeing them.

use std::path::{Path, PathBuf};

use crate::cli::{report_parse_error, FlagSet};
use crate::fsutil;
use crate::output::{self, Diagnostic, Mode, Report, Severity};

/// Why a focused gate stopped before it could report.
#[derive(Debug)]
pub enum Failure {
    /// The complaint is already on stderr; leave with this exit code.
    Exit(i32),
    /// `main` prints `Error: {0}` and exits 1.
    Fail(String),
}

impl Failure {
    /// Converts the failure into the dispatch result.
    pub fn into_result(self) -> Result<i32, String> {
        match self {
            Failure::Exit(code) => Ok(code),
            Failure::Fail(message) => Err(message),
        }
    }
}

/// One parsed focused command line.
#[derive(Debug)]
pub struct Focused {
    /// The parsed flags, including the gate's own.
    pub fs: FlagSet,
    root: PathBuf,
    output: String,
}

impl Focused {
    /// The repository the gate checks.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The output mode. An unset `--output` means TOON here, not text.
    pub fn mode(&self) -> Mode {
        if self.output.is_empty() {
            Mode::Toon
        } else {
            output::parse_mode(&self.output)
        }
    }

    /// The raw value of a registered string flag.
    pub fn string(&self, name: &str) -> String {
        self.fs
            .get(name)
            .map(|value| value.as_str(""))
            .unwrap_or_default()
    }

    /// The raw value of a registered boolean flag.
    pub fn bool(&self, name: &str) -> bool {
        self.fs
            .get(name)
            .map(|value| value.as_bool(false))
            .unwrap_or(false)
    }
}

/// The repository the command line names, before any flag is parsed.
///
/// Go's `focusedRoot` scans the raw arguments for `-project-root` and
/// resolves it against the working directory, because the flag set's default
/// has to be the absolute path the usage block prints.
pub fn root_default(args: &[String]) -> PathBuf {
    absolute(&literal(args, "project-root", "."))
}

/// The flags every focused gate shares: `-project-root` and `-output`.
pub fn base(root: &Path, args: &[String]) -> FlagSet {
    let mut output = literal(args, "output", "");
    if output.is_empty() {
        output = literal(args, "o", "");
    }
    let mut fs = FlagSet::new();
    fs.str_flag("project-root", root.to_string_lossy().as_ref())
        .str_flag("output", &output)
        // `-o` and `--output` write ONE Go variable; registering `o` as well
        // keeps it in the PrintDefaults block the usage error prints.
        .str_flag("o", &output)
        .alias("o", "output")
        .describe("project-root", "project root")
        .describe("output", "output: toon (default) | json | text")
        .describe("o", "--output shorthand");
    fs
}

/// Parses the command line and resolves the repository to check.
///
/// A refused command line prints the error, the gate's own usage block and
/// the `Error:` line, then exits 2; an unusable `--output` prints only the
/// `Error:` line and still exits 2.
pub fn finish(name: &str, args: &[String], mut fs: FlagSet) -> Result<Focused, Failure> {
    if let Err(error) = fs.parse(args) {
        report_parse_error(&error, &fs.render_usage(name));
        eprintln!("Error: {error}");
        return Err(Failure::Exit(2));
    }
    let output = fs
        .get("output")
        .map(|value| value.as_str(""))
        .unwrap_or_default();
    if let Err(message) = output::validate_mode(&output) {
        eprintln!("Error: {message}");
        return Err(Failure::Exit(2));
    }
    let root = fs
        .get("project-root")
        .map(|value| value.as_str(""))
        .unwrap_or_default();
    Ok(Focused {
        fs,
        root: absolute(&root),
        output,
    })
}

/// Writes one focused report the way Go's `renderLintReport` does.
///
/// Text mode prints the finding lines; the structured modes print the shared
/// [`Report`]. A gate with error-severity findings returns the message that
/// `fail()` prints on stderr after the payload.
pub fn render(
    command: &str,
    diagnostics: &[Diagnostic],
    extra: Option<serde_json::Value>,
    mode: Mode,
) -> Result<(), String> {
    let errors = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let summary = if errors == 0 {
        format!("{command}: OK")
    } else {
        format!("{command}: {errors} error(s)")
    };
    if mode == Mode::Text {
        print_findings(diagnostics, &summary);
        return verdict(command, errors);
    }
    let mut report = Report::new(command);
    report.summary = summary;
    report.extra = extra;
    for diagnostic in diagnostics {
        report.add(diagnostic.clone());
    }
    print!("{}", report.render(mode));
    verdict(command, errors)
}

/// The exit verdict: findings are an error, a clean report is not.
fn verdict(command: &str, errors: usize) -> Result<(), String> {
    if errors == 0 {
        Ok(())
    } else {
        Err(format!("{command} found {errors} error(s)"))
    }
}

/// The legacy text layout: one `severity location rule: message` line each.
fn print_findings(diagnostics: &[Diagnostic], summary: &str) {
    for d in diagnostics {
        let severity = d.severity.as_str();
        let mut location = d.path.clone();
        if !location.is_empty() && d.line > 0 {
            location = format!("{location}:{}", d.line);
        }
        if location.is_empty() {
            println!("  {severity:<5} {}: {}", d.rule, d.message);
        } else {
            println!("  {severity:<5} {location}  {}: {}", d.rule, d.message);
        }
    }
    if diagnostics.is_empty() {
        println!("  OK    {summary}");
    } else {
        println!("summary: {summary}");
    }
}

/// Scans the raw arguments for `-name value` and `-name=value`, the way Go's
/// `literalFlag` does, without ever refusing anything.
fn literal(args: &[String], name: &str, fallback: &str) -> String {
    let short = format!("-{name}");
    let long = format!("--{name}");
    for (index, arg) in args.iter().enumerate() {
        if arg == &short || arg == &long {
            if let Some(value) = args.get(index + 1) {
                return value.clone();
            }
            continue;
        }
        if let Some(rest) = arg.strip_prefix(&format!("{short}=")) {
            return rest.to_string();
        }
        if let Some(rest) = arg.strip_prefix(&format!("{long}=")) {
            return rest.to_string();
        }
    }
    fallback.to_string()
}

/// Resolves a path against the working directory the way `filepath.Abs`
/// does, falling back to the raw path when the directory is unknown.
fn absolute(path: &str) -> PathBuf {
    fsutil::absolute(Path::new(path)).unwrap_or_else(|_| PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_reads_both_spellings() {
        let args = vec!["--project-root".to_string(), "/tmp/x".to_string()];
        assert_eq!(literal(&args, "project-root", "."), "/tmp/x");
        let args = vec!["-o=json".to_string()];
        assert_eq!(literal(&args, "o", ""), "json");
        let args = vec!["-o".to_string()];
        assert_eq!(literal(&args, "o", ""), "");
        assert_eq!(literal(&[], "o", "fallback"), "fallback");
    }

    #[test]
    fn an_empty_output_means_toon() {
        let fs = base(Path::new("/tmp"), &[]);
        let flags = finish("go-pin", &[], fs).expect("parse");
        assert_eq!(flags.mode(), Mode::Toon);
        assert_eq!(flags.root(), Path::new("/tmp"));
    }

    #[test]
    fn a_refused_command_line_exits_two() {
        let fs = base(Path::new("/tmp"), &[]);
        let failure = finish("go-pin", &["--nope".to_string()], fs).expect_err("refused");
        assert!(matches!(failure, Failure::Exit(2)));
    }
}
