// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Running the toolchain of a language whose gates shell out.
//!
//! The Go and Rust gates drive their own toolchains; the groups this module
//! serves (Python and Mojo) drive *someone else's*, which is a different
//! contract in three ways:
//!
//! * the tool may not be installed, and that is a configuration error the
//!   caller has to be told about by name, not a crash;
//! * the tool's findings arrive as `path:line:col: message` lines on stdout,
//!   so they are turned into the same diagnostics every other gate emits;
//! * a tool that never ran must not look like a gate that passed.
//!
//! Nothing here decides *which* gate runs — the language modules do that.

use std::path::PathBuf;

use crate::config::Config;
use crate::output::{Diagnostic, Report};
use crate::runner::{self, CmdOutput, Runner};

/// The longest tail of a tool's output a failing gate keeps in its detail.
const OUTPUT_TAIL_LINES: usize = 40;

/// Where a program lives, when it is installed at all.
pub fn find(program: &str) -> Option<PathBuf> {
    if program.is_empty() {
        return None;
    }
    // A path the caller spelled out is used as-is: `--tool ./venv/bin/ruff`
    // is how a project pins the exact binary its CI uses.
    if program.contains('/') {
        let path = PathBuf::from(program);
        return path.is_file().then_some(path);
    }
    if !Runner::command_exists(program) {
        return None;
    }
    Some(PathBuf::from(program))
}

/// Runs one tool, or explains why it could not run.
///
/// `hint` is the sentence a caller adds to the "not installed" complaint —
/// it is the language's own install line, so the reader is told what to do
/// rather than only what is missing.
pub fn run(cfg: &Config, program: &str, args: &[String], hint: &str) -> Result<CmdOutput, String> {
    // A dry run is a plan, not a gate: it says what would run, so it does not
    // need the tool to be installed to be useful.
    if cfg.dry_run {
        println!("[dry-run] {program} {}", args.join(" "));
        return Ok(CmdOutput {
            output: String::new(),
            stdout: String::new(),
            stderr: String::new(),
            success: true,
            code: Some(0),
        });
    }
    if find(program).is_none() {
        return Err(format!("{program} not found on PATH — {hint}"));
    }
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let runner = Runner::new(cfg.root(), cfg.verbose);
    let out = runner.run_with_timeout(runner::DEFAULT_CMD_TIMEOUT, program, &borrowed)?;
    if cfg.verbose {
        print!("{}", out.output);
    }
    Ok(out)
}

/// Turns a tool's exit status into a gate verdict.
///
/// A passing tool reports the line it printed; a failing one turns each
/// `path:line:col: message` into a diagnostic, so `--output json` carries
/// the findings rather than a wall of text.
pub fn verdict(
    cfg: &Config,
    gate: &str,
    ok_summary: impl Into<String>,
    out: &CmdOutput,
) -> crate::gates::GateResult {
    let mut report = Report::new(gate);
    if out.success {
        // A dry run gates nothing, so its summary says what would have run
        // rather than claiming a pass nobody made.
        let summary = ok_summary.into();
        report.summary = if cfg.dry_run {
            format!("{gate}: would run ({summary})")
        } else {
            format!("{gate}: {summary}")
        };
        return crate::gates::GateResult::finish(report);
    }
    for finding in parse_findings(&out.output) {
        report.add(finding);
    }
    if report.ok {
        // The tool failed without a line this gate could attribute: its own
        // words are the finding, so the reader is never shown an empty pass.
        // The finding is the sentence to act on, not the last line of a
        // wall: a tool's first line is often its usage banner and its last a
        // path inside a report table.
        report.add(Diagnostic::error(
            "",
            format!("{gate}.tool"),
            blame_of(&out.output),
        ));
    }
    let found = report.diagnostics.len();
    let exit = out
        .code
        .map(|c| c.to_string())
        .unwrap_or_else(|| "?".into());
    let summary = format!("{gate}: {found} finding(s) (exit {exit})");
    report.summary = summary.clone();
    crate::gates::GateResult::failed(gate, summary, report)
        .failing(tail_of(&out.output), format!("{found} finding(s)"))
}

/// Parses `path:line:col: message` (and `path:line: message`) findings.
///
/// Ruff and mypy both print this shape; a line that does not match is not a
/// finding and is left to the tail.
pub fn parse_findings(text: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some((location, message)) = line.split_once(": ") else {
            continue;
        };
        let parts: Vec<&str> = location.rsplitn(3, ':').collect();
        let Some(path) = parts.last().copied() else {
            continue;
        };
        if path.is_empty() || path.contains(' ') {
            continue;
        }
        let (line_no, col) = match parts.as_slice() {
            [col, line_no, _] => (line_no.parse().unwrap_or(0), col.parse().unwrap_or(0)),
            [line_no, _] => (line_no.parse().unwrap_or(0), 0),
            _ => (0, 0),
        };
        if line_no == 0 {
            continue;
        }
        out.push(
            Diagnostic::error(path, "finding", message.trim())
                .at_line(line_no)
                .at_col(col),
        );
    }
    out
}

/// The last few lines of a tool's output, which is what a reader needs.
pub fn tail_of(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(OUTPUT_TAIL_LINES);
    lines[start..].join("\n")
}

/// The one line of a failing tool's output that says what went wrong.
///
/// A tool that fails without printing `path:line:col` still has a sentence a
/// reader can act on somewhere in its output: pytest puts a coverage failure
/// under its own summary table, and a usage error sits under a banner. That
/// sentence is the first line naming an error; with none to find, the first
/// non-empty line is the next best thing, and a silent tool still says
/// something rather than nothing.
pub fn blame_of(text: &str) -> String {
    let mut first = "";
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if first.is_empty() {
            first = line;
        }
        if line.to_lowercase().contains("error") {
            return line.to_string();
        }
    }
    if first.is_empty() {
        "the tool failed without a message".to_string()
    } else {
        first.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blame_is_the_line_that_names_an_error() {
        let wall = "usage: pytest [options]\n.\n".to_string()
            + "ERROR: Coverage failure: total of 86 is less than fail-under=100\n"
            + "  Name  Stmts  Cover\n";
        assert_eq!(
            blame_of(&wall),
            "ERROR: Coverage failure: total of 86 is less than fail-under=100"
        );
    }

    #[test]
    fn a_silent_tool_still_says_something() {
        assert_eq!(blame_of("   \n\n"), "the tool failed without a message");
    }

    #[test]
    fn parses_ruff_style_findings() {
        let findings = parse_findings(
            "src/app.py:12:5: E501 Line too long\nsrc/db.py:3:1: F401 unused import\n",
        );
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].path, "src/app.py");
        assert_eq!(findings[0].line, 12);
        assert_eq!(findings[0].col, 5);
        assert!(findings[0].message.contains("Line too long"));
        assert_eq!(findings[1].path, "src/db.py");
    }

    #[test]
    fn parses_two_part_findings() {
        let findings = parse_findings("pkg/mod.py:7: something is wrong\n");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 7);
        assert_eq!(findings[0].col, 0);
    }

    #[test]
    fn ignores_lines_without_a_location() {
        assert!(parse_findings("Found 3 errors\n").is_empty());
        assert!(parse_findings("").is_empty());
    }

    #[test]
    fn a_missing_tool_says_what_to_install() {
        let cfg = Config::new();
        let err = run(&cfg, "definitely-not-installed-xyz", &[], "install it").unwrap_err();
        assert!(err.contains("not found on PATH"), "{err}");
        assert!(err.contains("install it"), "{err}");
    }
}
