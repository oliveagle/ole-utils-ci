// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `test-report` subcommand: `go test -json` to JUnit XML.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::config::Config;
use crate::runner::Runner;

/// The `test-report` subcommand: turn the test output into JUnit XML.
pub fn test_report(cfg: &Config) -> Result<i32, String> {
    println!("📝 Generating the JUnit report...");
    let runner = Runner::new(cfg.root(), cfg.verbose);
    let result =
        runner.run_with_timeout(Duration::from_secs(3600), "go", &["test", "-json", "./..."])?;
    let xml = junit_from_go_json(&result.output);
    std::fs::write(&cfg.junit_path, &xml)
        .map_err(|e| format!("failed to write {}: {e}", cfg.junit_path))?;
    println!("✅ JUnit report written to {}", cfg.junit_path);
    Ok(0)
}

/// Builds a JUnit XML document from `go test -json` output.
pub fn junit_from_go_json(output: &str) -> String {
    let mut suites: BTreeMap<String, (usize, usize, Vec<String>)> = BTreeMap::new();
    for line in output.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(package) = value.get("Package").and_then(|v| v.as_str()) else {
            continue;
        };
        let action = value.get("Action").and_then(|v| v.as_str()).unwrap_or("");
        let test = value.get("Test").and_then(|v| v.as_str());
        let entry = suites
            .entry(package.to_string())
            .or_insert((0, 0, Vec::new()));
        match (action, test) {
            ("pass", Some(name)) => {
                entry.0 += 1;
                entry.2.push(format!(
                    "    <testcase classname=\"{package}\" name=\"{name}\"/>"
                ));
            }
            ("fail", Some(name)) => {
                entry.1 += 1;
                entry.2.push(format!(
                    "    <testcase classname=\"{package}\" name=\"{name}\"><failure/></testcase>"
                ));
            }
            _ => {}
        }
    }
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites>\n");
    for (package, (passed, failed, cases)) in suites {
        xml.push_str(&format!(
            "  <testsuite name=\"{package}\" tests=\"{}\" failures=\"{}\">\n",
            passed + failed,
            failed
        ));
        for case in cases {
            xml.push_str(&case);
            xml.push('\n');
        }
        xml.push_str("  </testsuite>\n");
    }
    xml.push_str("</testsuites>\n");
    xml
}
