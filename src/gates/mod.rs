// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The quality gates.
//!
//! Every gate follows the same contract:
//!
//! * It reads the repository under `--project-root`.
//! * It renders human text by default and a structured [`Report`] when the
//!   caller passes `--output json` or `--output toon`.
//! * It exits non-zero when it finds anything that must be fixed.
//!
//! Gates that only *warn* (package size) still exit zero; gates that
//! *fail* (bugs, license, structure) exit one.

pub mod artifact_size;
pub mod bugs;
pub mod complexity;
pub mod file_length;
pub mod focused;
pub mod gopin;
pub mod goversion;
pub mod license;
pub mod noshell;
pub mod package_size;
pub mod plugin_freshness;
pub mod sanity;
pub mod structure;

use crate::config::Config;
use crate::output::Report;

/// The outcome of a gate: a rendered payload plus a verdict.
#[derive(Debug, Clone)]
pub struct GateResult {
    /// The structured report, always populated.
    pub report: Report,
    /// The message Go's text renderer returns; `main` prints it as `Error:`.
    pub text_error: Option<String>,
    /// The message Go's structured renderer returns.
    pub structured_error: Option<String>,
}

impl GateResult {
    /// Wraps an already-populated report. The report carries the command name,
    /// the diagnostics, the `extra` payload and its own `ok` verdict.
    pub fn finish(report: Report) -> Self {
        GateResult {
            report,
            text_error: None,
            structured_error: None,
        }
    }

    /// Builds a failing result with the given summary and diagnostics.
    pub fn failed(command: &str, summary: impl Into<String>, report: Report) -> Self {
        let mut report = report;
        report.command = command.to_string();
        report.summary = summary.into();
        report.ok = false;
        GateResult::finish(report)
    }

    /// Records the two `Error:` messages a failing gate returns.
    ///
    /// Go's text and structured renderers return different complaints for the
    /// same failure — one names the files, the other counts them — so both
    /// are kept and `emit` picks the one that matches the output mode.
    pub fn failing(mut self, text: impl Into<String>, structured: impl Into<String>) -> Self {
        self.text_error = Some(text.into());
        self.structured_error = Some(structured.into());
        self
    }

    /// The complaint to print on stderr, if this run has one.
    pub fn error_message(&self, structured: bool) -> Option<&str> {
        if structured {
            self.structured_error.as_deref()
        } else {
            self.text_error.as_deref()
        }
    }

    /// Whether the gate passed.
    pub fn passed(&self) -> bool {
        self.report.ok
    }
}

/// The line a gate prints when it had nothing to check.
///
/// A gate that skips puts its reason in the summary rather than in a
/// diagnostic, so the legacy text must not claim a pass it never made.
pub fn skipped_line(summary: &str) -> Option<String> {
    summary
        .ends_with("skipped")
        .then(|| format!("⏭ {summary}\n"))
}
/// Prints a gate result in the requested format and returns the exit code.
pub fn emit(cfg: &Config, result: &GateResult) -> i32 {
    let mode = cfg.output_mode();
    if cfg.structured() {
        print!("{}", result.report.render(mode));
    } else {
        print!("{}", result.report.render_text());
    }
    report_error(cfg, result)
}

/// Prints a gate result using the legacy human text, which some scripts grep
/// for. `body` is the text the Go version printed before its structured
/// rewrite.
pub fn emit_text(cfg: &Config, result: &GateResult, body: &str) -> i32 {
    if cfg.structured() {
        print!("{}", result.report.render(cfg.output_mode()));
    } else {
        print!("{body}");
    }
    report_error(cfg, result)
}

/// Prints the `Error:` line Go's `fail()` writes and returns the exit code.
fn report_error(cfg: &Config, result: &GateResult) -> i32 {
    match result.error_message(cfg.structured()) {
        Some(message) => {
            eprintln!("Error: {message}");
            1
        }
        None if result.passed() => 0,
        None => 1,
    }
}
