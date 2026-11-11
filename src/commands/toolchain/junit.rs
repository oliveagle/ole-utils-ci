// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `test-report` subcommand: `go test -json` to JUnit XML.
//!
//! The Go original runs the tests with the JSON event stream, reads the
//! `pass` and `fail` events of the named tests and writes one `<testsuite>`
//! per package. A failing test run still produces a report — the warning
//! says so — because a report of a red run is exactly what a CI job needs.

mod xml;

use std::collections::BTreeMap;
use std::time::Duration;

use crate::config::Config;
use crate::fsutil;
use crate::runner::Runner;

use super::install;

/// The wall-clock budget the test run gets.
const REPORT_TIMEOUT: Duration = Duration::from_secs(900);

/// The timestamp of an event that carries none.
const ZERO_TIME: &str = "0001-01-01T00:00:00Z";

/// One test suite: the package the tests live in.
#[derive(Debug, Clone)]
pub struct Suite {
    /// The package the tests live in.
    pub name: String,
    /// The timestamp of the first event the package reported.
    pub timestamp: String,
    /// The number of test cases.
    pub tests: usize,
    /// The number of failed test cases.
    pub failures: usize,
    /// The test cases themselves.
    pub cases: Vec<TestCase>,
}

/// One test case.
#[derive(Debug, Clone)]
pub struct TestCase {
    /// The package the test lives in.
    pub classname: String,
    /// The name of the test.
    pub name: String,
    /// How long the test took, in seconds.
    pub time: f64,
    /// Whether the test failed.
    pub failed: bool,
}

/// The `test-report` subcommand: turn the test output into JUnit XML.
pub fn test_report(cfg: &Config) -> Result<i32, String> {
    println!("📝 Generating test report...");
    install::install_dependencies(cfg)
        .map_err(|e| format!("test report failed: dependency install error: {e}"))?;
    let runner = Runner::new(cfg.root(), cfg.verbose);
    let result = runner.run_go(REPORT_TIMEOUT, "go", &["test", "-json", "./..."]);
    if let Some(error) = &result.error {
        println!("  ⚠ Tests failed, generating report anyway: {error}");
    }
    let suites = parse_test_output(&result.output);
    let document = xml::render(&suites);
    let path = fsutil::go_join(cfg.root(), &cfg.junit_path);
    std::fs::write(&path, document).map_err(|e| {
        format!(
            "failed to write JUnit report: open {}: {}",
            path.display(),
            crate::commands::ci_recipe_util::go_errno(&e)
        )
    })?;
    println!("  ✓ JUnit report: {}", cfg.junit_path);
    println!("✅ Test report generated");
    Ok(0)
}

/// Reads the `go test -json` events into test suites.
///
/// Only the `pass` and `fail` actions of a named test count; the package
/// level events are skipped. The suites come back in the sorted order of
/// their package, so two runs over the same module produce the same report.
pub fn parse_test_output(output: &str) -> Vec<Suite> {
    let mut suites: BTreeMap<String, Suite> = BTreeMap::new();
    for line in output.split('\n') {
        if line.is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(action) = text(&event, "Action") else {
            continue;
        };
        let Some(test) = text(&event, "Test") else {
            continue;
        };
        let Some(package) = text(&event, "Package") else {
            continue;
        };
        let Some(elapsed) = number(&event, "Elapsed") else {
            continue;
        };
        let Some(time) = text(&event, "Time") else {
            continue;
        };
        let action = action.unwrap_or("");
        if action != "pass" && action != "fail" {
            continue;
        }
        let test = test.unwrap_or("");
        if test.is_empty() {
            continue;
        }
        let Some(timestamp) = timestamp(time) else {
            continue;
        };
        let package = package.unwrap_or("").to_string();
        let failed = action == "fail";
        let suite = suites.entry(package.clone()).or_insert_with(|| Suite {
            name: package.clone(),
            timestamp,
            tests: 0,
            failures: 0,
            cases: Vec::new(),
        });
        suite.tests += 1;
        if failed {
            suite.failures += 1;
        }
        suite.cases.push(TestCase {
            classname: package,
            name: test.to_string(),
            time: elapsed.unwrap_or(0.0),
            failed,
        });
    }
    suites.into_values().collect()
}

/// The JUnit XML document for a `go test -json` stream.
pub fn junit_from_go_json(output: &str) -> String {
    xml::render(&parse_test_output(output))
}

/// The RFC3339 spelling of an event timestamp.
///
/// Go parses the event time and prints it back in RFC3339, which drops the
/// fractional seconds and keeps the offset.
fn timestamp(raw: Option<&str>) -> Option<String> {
    let Some(raw) = raw else {
        return Some(ZERO_TIME.to_string());
    };
    let parsed = chrono::DateTime::parse_from_rfc3339(raw).ok()?;
    Some(parsed.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// One field of an event, as text.
///
/// `None` means the field is absent, and the outer `None` means the field
/// has the wrong type — which makes the whole event unusable, the way Go's
/// `json.Unmarshal` refuses it.
fn text<'a>(event: &'a serde_json::Value, name: &str) -> Option<Option<&'a str>> {
    match event.get(name) {
        None => Some(None),
        Some(serde_json::Value::String(value)) => Some(Some(value.as_str())),
        Some(_) => None,
    }
}

/// One field of an event, as a number.
fn number(event: &serde_json::Value, name: &str) -> Option<Option<f64>> {
    match event.get(name) {
        None => Some(None),
        Some(serde_json::Value::Number(value)) => Some(value.as_f64()),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests;
