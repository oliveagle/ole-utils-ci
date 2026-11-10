// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `goversion` gate: the `go` directive in `go.mod` must match the
//! toolchain that builds the project.
//!
//! The check is skipped when the Go toolchain is not installed, so the gate
//! never blocks a machine that does not build Go code.

use regex::Regex;

use crate::config::Config;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};

/// The `go x.y` directive in a `go.mod`.
pub fn go_directive(content: &str) -> Option<String> {
    let re = Regex::new(r"(?m)^go\s+(\d+\.\d+(?:\.\d+)?)").expect("static go regex must compile");
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// The version of the Go toolchain on `PATH`.
pub fn runtime_version() -> Option<String> {
    let output = std::process::Command::new("go")
        .arg("version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let re = Regex::new(r"go(\d+\.\d+(?:\.\d+)?)").expect("static version regex must compile");
    re.captures(&text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Compares two Go versions by their `x.y` prefix.
pub fn same_minor(a: &str, b: &str) -> bool {
    minor(a) == minor(b)
}

fn minor(version: &str) -> String {
    version.split('.').take(2).collect::<Vec<&str>>().join(".")
}

/// Runs the gate and returns its result.
pub fn run(cfg: &Config) -> Result<GateResult, String> {
    let gomod = cfg.root().join("go.mod");
    let mut report = Report::new("goversion");
    let Ok(content) = std::fs::read_to_string(&gomod) else {
        report.summary = "goversion: no go.mod, skipped".to_string();
        return Ok(GateResult::finish(report));
    };
    let Some(directive) = go_directive(&content) else {
        report.summary = "goversion: no go directive in go.mod, skipped".to_string();
        return Ok(GateResult::finish(report));
    };
    let Some(runtime) = runtime_version() else {
        report.summary = "goversion: go toolchain not installed, skipped".to_string();
        return Ok(GateResult::finish(report));
    };
    report.extra = Some(serde_json::json!({
        "go_directive": directive,
        "runtime_version": runtime,
    }));
    if !same_minor(&directive, &runtime) {
        report.add(Diagnostic::error(
            "go.mod",
            "goversion/mismatch",
            format!("go.mod declares go {directive}, toolchain is {runtime}"),
        ));
    }
    if report.ok {
        report.summary = format!("goversion: go {directive} matches the toolchain");
        Ok(GateResult::finish(report))
    } else {
        report.summary = "goversion: version mismatch".to_string();
        Ok(GateResult::failed(
            "goversion",
            report.summary.clone(),
            report,
        ))
    }
}

/// The human-readable body printed when `--output` is not set.
pub fn text_body(result: &GateResult) -> String {
    let mut body = String::from("🐹 Checking Go version...\n");
    for d in &result.report.diagnostics {
        body.push_str(&format!("  ✗ {}: {}\n", d.path, d.message));
    }
    if result.passed() {
        body.push_str("✅ Go version matches go.mod\n");
    } else {
        body.push_str("✗ Go version mismatch\n");
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_go_directive() {
        assert_eq!(
            go_directive("module x\n\ngo 1.25.11\n"),
            Some("1.25.11".to_string())
        );
        assert_eq!(go_directive("module x\n"), None);
    }

    #[test]
    fn compares_minor_versions() {
        assert!(same_minor("1.25.11", "1.25.0"));
        assert!(!same_minor("1.25.11", "1.24.0"));
    }
}
