// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `moon` toolchain bridge.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::fsutil;
use crate::output::Severity;
use crate::runner::Runner;

use super::support::{inside_build, temp_path};

pub(crate) use super::support::first_line;

/// One compiler diagnostic from `moon check --json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoonDiagnostic {
    /// Absolute source path; empty for a module-level diagnostic.
    pub file: String,
    /// 1-based line.
    pub line: usize,
    /// 1-based column.
    pub col: usize,
    /// Error or warning.
    pub severity: Severity,
    /// Compiler error code, when present.
    pub code: String,
    /// The compiler's message.
    pub message: String,
}

/// One failed test from `moon test --test-failure-json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestFailure {
    /// Test source file.
    pub file: String,
    /// Test name.
    pub name: String,
    /// The failure message.
    pub message: String,
}

/// Totals printed by `moon test`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TestTotals {
    /// Every test moon ran.
    pub total: usize,
    /// Tests that passed.
    pub passed: usize,
    /// Tests that failed.
    pub failed: usize,
}

/// The outcome of one external command.
#[derive(Debug, Clone)]
pub struct ToolRun {
    /// Whether the command exited zero.
    pub success: bool,
    /// Standard output.
    pub stdout: String,
    /// Combined output.
    pub output: String,
}

/// Whether the `moon` executable is reachable.
pub fn tool_available() -> bool {
    Command::new("moon")
        .arg("version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Finds the module root: `src`, or its nearest ancestor holding `moon.mod`.
pub fn module_root(src: &Path) -> Option<PathBuf> {
    src.ancestors()
        .find(|dir| dir.join("moon.mod").is_file())
        .map(Path::to_path_buf)
}

/// Runs `moon check --json`.
pub fn check(module: &Path, timeout: Duration, deny_warnings: bool) -> Result<ToolRun, String> {
    let mut args = vec!["check", "--json"];
    if deny_warnings {
        args.push("--deny-warn");
    }
    command(module, timeout, &args)
}

/// Runs `moon fmt --check`.
pub fn format_check(module: &Path, timeout: Duration) -> Result<ToolRun, String> {
    command(module, timeout, &["fmt", "--check"])
}

/// Runs `moon test`, optionally with coverage instrumentation and a filter.
pub fn test(
    module: &Path,
    timeout: Duration,
    coverage: bool,
    filter: &str,
) -> Result<ToolRun, String> {
    let mut args = vec!["test", "--test-failure-json"];
    if coverage {
        args.push("--enable-coverage");
    }
    if !filter.is_empty() {
        args.extend(["--filter", filter]);
    }
    command(module, timeout, &args)
}

/// Writes one Cobertura report and returns its text.
pub fn coverage_report(module: &Path, timeout: Duration) -> Result<String, String> {
    let output = temp_path("xml");
    let text = output.to_string_lossy().into_owned();
    let args = ["coverage", "report", "-f", "cobertura", "-o", text.as_str()];
    let run = command(module, timeout, &args)?;
    let report = std::fs::read_to_string(&output);
    let _ = std::fs::remove_file(&output);
    if !run.success {
        return Err(first_line(&run.output));
    }
    report.map_err(|error| format!("read Cobertura report: {error}"))
}

/// Every MoonBit source under `root`, ignoring build directories.
pub fn discover_sources(root: &Path) -> Vec<fsutil::Entry> {
    fsutil::walk_files(root)
        .into_iter()
        .filter(|entry| entry.extension() == ".mbt" && !inside_build(&entry.rel))
        .collect()
}

/// Parses the JSON object `moon check --json` writes to standard output.
pub fn parse_check(stdout: &str) -> Vec<MoonDiagnostic> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(stdout.trim()) else {
        return Vec::new();
    };
    let Some(diagnostics) = value.get("diagnostics").and_then(|item| item.as_array()) else {
        return Vec::new();
    };
    diagnostics
        .iter()
        .map(|item| {
            let (line, col) = item
                .get("loc")
                .and_then(|loc| loc.as_str())
                .map(parse_loc)
                .unwrap_or((0, 0));
            MoonDiagnostic {
                file: string_field(item, "path"),
                line,
                col: if line == 0 { 0 } else { col + 1 },
                severity: match item.get("level").and_then(|level| level.as_str()) {
                    Some("warning") => Severity::Warning,
                    Some("info") => Severity::Info,
                    _ => Severity::Error,
                },
                code: item
                    .get("error_code")
                    .map(|code| code.to_string())
                    .unwrap_or_default(),
                message: string_field(item, "message"),
            }
        })
        .collect()
}

/// Reads the `Total tests: N, passed: N, failed: N.` line.
pub fn parse_test_totals(stdout: &str) -> TestTotals {
    for line in stdout.lines() {
        let Some(rest) = line.trim().strip_prefix("Total tests:") else {
            continue;
        };
        let mut parts = rest.split(',');
        let mut totals = TestTotals {
            total: parts
                .next()
                .unwrap_or_default()
                .trim()
                .trim_end_matches('.')
                .parse()
                .unwrap_or(0),
            ..TestTotals::default()
        };
        for part in parts {
            let Some((name, value)) = part.split_once(':') else {
                continue;
            };
            let value = value.trim().trim_end_matches('.').parse().unwrap_or(0);
            match name.trim() {
                "passed" => totals.passed = value,
                "failed" => totals.failed = value,
                _ => {}
            }
        }
        return totals;
    }
    TestTotals::default()
}

/// Parses `--test-failure-json` lines.
pub fn parse_test_failures(stdout: &str) -> Vec<TestFailure> {
    let mut failures = Vec::new();
    for line in stdout.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
            continue;
        };
        if value.get("test_name").is_none() {
            continue;
        }
        failures.push(TestFailure {
            file: string_field(&value, "filename"),
            name: string_field(&value, "test_name"),
            message: string_field(&value, "message"),
        });
    }
    failures
}

/// The first meaningful line of a tool output.
fn command(module: &Path, timeout: Duration, args: &[&str]) -> Result<ToolRun, String> {
    Runner::new(module, false)
        .run_with_timeout(timeout, "moon", args)
        .map(|out| ToolRun {
            success: out.success,
            stdout: out.stdout,
            output: out.output,
        })
}

fn parse_loc(loc: &str) -> (usize, usize) {
    let start = loc.split('-').next().unwrap_or(loc);
    let mut parts = start.split(':');
    let line = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let col = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    (line, col)
}

fn string_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_check_diagnostics_and_locations() {
        let text = r#"{"version":1,"status":"failure","diagnostics":[{"level":"error","error_code":3002,"path":"/tmp/a.mbt","loc":"3:7-3:9","message":"Parse error"}],"summary":{}}"#;
        let diagnostics = parse_check(text);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert_eq!(diagnostics[0].col, 8);
        assert_eq!(diagnostics[0].code, "3002");
    }

    #[test]
    fn parses_test_totals_and_failures() {
        let text = "{\"filename\":\"a_test.mbt\",\"test_name\":\"fails\",\"message\":\"bad\"}\nTotal tests: 2, passed: 1, failed: 1.\n";
        let totals = parse_test_totals(text);
        assert_eq!(
            totals,
            TestTotals {
                total: 2,
                passed: 1,
                failed: 1
            }
        );
        let failures = parse_test_failures(text);
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].name, "fails");
    }
}
