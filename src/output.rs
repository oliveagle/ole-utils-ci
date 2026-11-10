// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Structured output for every machine-consumable subcommand.
//!
//! Three modes are supported, matching the Go tool's contract:
//!
//! * `text` — human-oriented; one `SEV path:line:col  rule: message` line per
//!   diagnostic, preceded by a one-sentence summary.
//! * `json` — canonical JSON, two-space indent.
//! * `toon` — Token-Oriented Object Notation, a compact JSON/YAML hybrid that
//!   costs an LLM roughly half the tokens of pretty JSON.
//!
//! JSON is the reference format: TOON is produced by round-tripping the
//! payload through `serde_json::Value` first, so both modes always expose the
//! exact same keys. That invariant is what lets an agent that can parse one
//! mode parse the other.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The output format selected by `--output`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Free-form human output.
    Text,
    /// Canonical JSON.
    Json,
    /// Token-Oriented Object Notation.
    Toon,
}

impl Mode {
    /// Canonical CLI spelling of this mode.
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Text => "text",
            Mode::Json => "json",
            Mode::Toon => "toon",
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Maps a `--output` flag value to a [`Mode`].
///
/// Unknown values fall back to [`Mode::Text`] so the tool stays usable even
/// when a caller passes something unexpected; use [`validate_mode`] when the
/// value must be rejected instead.
pub fn parse_mode(raw: &str) -> Mode {
    match raw.trim().to_ascii_lowercase().as_str() {
        "json" => Mode::Json,
        "toon" => Mode::Toon,
        _ => Mode::Text,
    }
}

/// Rejects `--output` values outside `{"", "text", "json", "toon"}`.
pub fn validate_mode(raw: &str) -> Result<(), String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "text" | "json" | "toon" => Ok(()),
        // Go quotes the value exactly as the caller spelled it, before the
        // case fold that decides whether it is acceptable.
        _ => Err(format!(
            "unknown --output value {raw:?} — use text, json, or toon"
        )),
    }
}

/// How severe a single finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Fails the check (exit code 1).
    Error,
    /// Surfaced, but does not fail the check on its own.
    Warning,
    /// Advisory only.
    Info,
}

impl Severity {
    /// Canonical lowercase name.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

/// One check finding: what file, where, which rule, what to tell the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// How the finding should be treated.
    pub severity: Severity,
    /// Path relative to the project root.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    /// 1-based line number, when the finding is line-specific.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub line: usize,
    /// 1-based column number, when the finding is column-specific.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub col: usize,
    /// Stable rule identifier.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub rule: String,
    /// One-sentence description of the finding.
    pub message: String,
    /// Optional free-text second sentence.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

fn is_zero(v: &usize) -> bool {
    *v == 0
}

impl Diagnostic {
    /// Builds an error diagnostic.
    pub fn error(
        path: impl Into<String>,
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            severity: Severity::Error,
            path: path.into(),
            line: 0,
            col: 0,
            rule: rule.into(),
            message: message.into(),
            detail: String::new(),
        }
    }

    /// Builds a warning diagnostic.
    pub fn warning(
        path: impl Into<String>,
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            path: path.into(),
            line: 0,
            col: 0,
            rule: rule.into(),
            message: message.into(),
            detail: String::new(),
        }
    }

    /// Builds an info diagnostic.
    pub fn info(
        path: impl Into<String>,
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            severity: Severity::Info,
            path: path.into(),
            line: 0,
            col: 0,
            rule: rule.into(),
            message: message.into(),
            detail: String::new(),
        }
    }

    /// Attaches a 1-based line number.
    pub fn at_line(mut self, line: usize) -> Self {
        self.line = line;
        self
    }
}

/// Tally of findings by severity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReportCounts {
    /// Number of error-severity findings.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub errors: usize,
    /// Number of warning-severity findings.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub warnings: usize,
    /// Number of info-severity findings.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub infos: usize,
}

fn is_zero_usize(v: &usize) -> bool {
    *v == 0
}

/// The shared "diagnostics + verdict" payload every linter-style subcommand
/// emits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// The CLI verb that produced this report.
    pub command: String,
    /// Pass/fail verdict. `false` means the command exits non-zero.
    pub ok: bool,
    /// One short human-readable sentence.
    pub summary: String,
    /// Tally of findings by severity.
    pub counts: ReportCounts,
    /// Findings, in the order the check produced them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
    /// Command-specific structured data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

impl Report {
    /// Creates an empty, passing report for `command`.
    pub fn new(command: &str) -> Self {
        Report {
            command: command.to_string(),
            ok: true,
            summary: String::new(),
            counts: ReportCounts::default(),
            diagnostics: Vec::new(),
            extra: None,
        }
    }

    /// Attaches command-specific structured data.
    pub fn with_extra(mut self, extra: serde_json::Value) -> Self {
        self.extra = Some(extra);
        self
    }

    /// Records a finding and updates the verdict and the tally.
    pub fn add(&mut self, diagnostic: Diagnostic) {
        match diagnostic.severity {
            Severity::Error => {
                self.counts.errors += 1;
                self.ok = false;
            }
            Severity::Warning => self.counts.warnings += 1,
            Severity::Info => self.counts.infos += 1,
        }
        self.diagnostics.push(diagnostic);
    }
}

pub mod render;

#[cfg(test)]
mod tests;
