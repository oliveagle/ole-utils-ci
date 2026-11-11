// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `goversion` gate: the running Go toolchain must satisfy `go.mod`.
//!
//! The comparison is numeric and three-part, so a newer toolchain passes:
//! `go 1.22.2` satisfies `go 1.21`. A repository without `go.mod` — every
//! Rust project — and a machine without a `go` binary have nothing to
//! compare, so both are reported as a skip rather than a failure.

use std::path::Path;

use crate::config::Config;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report, Severity};
use crate::static_regex;

/// One observation of the project's Go version requirements.
#[derive(Debug, Default, Clone)]
struct Info {
    /// The version the `go` directive asks for.
    gomod_go: String,
    /// The version the toolchain on `PATH` reports.
    running_go: String,
    /// The `GOTOOLCHAIN` setting in the environment.
    toolchain: String,
    /// Whether the running toolchain satisfies the directive.
    ok: bool,
    /// Why an unsatisfied directive cannot fix itself.
    warning: String,
    /// Whether `go.mod` has no `go` directive at all.
    no_directive: bool,
}

/// The `go x.y` directive in a `go.mod`.
pub fn go_directive(content: &str) -> Option<String> {
    let re = static_regex::compile(r"(?m)^go\s+(\d+\.\d+(?:\.\d+)?)");
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// The version of the Go toolchain on `PATH`, or `None` when there is none.
pub fn runtime_version() -> Option<String> {
    let output = std::process::Command::new("go")
        .arg("version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    let version = parts[2].trim_start_matches("go");
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

/// Compares two Go versions numerically, the way the Go original does.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    parse_version(a).cmp(&parse_version(b))
}

/// The first three numeric components of a version, missing ones as zero.
fn parse_version(version: &str) -> [u32; 3] {
    let mut out = [0u32; 3];
    for (index, part) in version.split('.').take(3).enumerate() {
        let mut value = 0u32;
        for c in part.chars() {
            let Some(digit) = c.to_digit(10) else {
                break;
            };
            value = value.saturating_mul(10).saturating_add(digit);
        }
        out[index] = value;
    }
    out
}

/// Runs the gate and returns its result.
pub fn run(cfg: &Config) -> Result<GateResult, String> {
    let Some(info) = gather(cfg.root())? else {
        let mut report = Report::new("goversion");
        report.summary = "goversion: no Go module or toolchain, skipped".to_string();
        return Ok(GateResult::finish(report));
    };
    let mut report = Report::new("goversion");
    report.extra = Some(extra(&info));
    if info.no_directive {
        report.summary = "goversion: no go directive in go.mod (skipped)".to_string();
        return Ok(GateResult::finish(report));
    }
    if !info.warning.is_empty() {
        report.add(Diagnostic::warning("", "toolchain", info.warning.clone()));
    }
    if info.ok {
        report.summary = format!(
            "goversion: running go {} ≥ go.mod requirement go {}",
            info.running_go, info.gomod_go
        );
        return Ok(GateResult::finish(report));
    }
    report.summary = format!(
        "goversion: running go {} < go.mod requirement go {}",
        info.running_go, info.gomod_go
    );
    report.add(Diagnostic::error("", "goversion", report.summary.clone()));
    let text = format!(
        "running Go {} is older than go.mod requirement go {}",
        info.running_go, info.gomod_go
    );
    Ok(
        GateResult::failed("goversion", report.summary.clone(), report)
            .failing(text, "go version mismatch"),
    )
}

/// The `extra` payload, with its keys in the sorted order Go's map encodes.
fn extra(info: &Info) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("gomod_go".to_string(), serde_json::json!(info.gomod_go));
    map.insert("running_go".to_string(), serde_json::json!(info.running_go));
    map.insert("toolchain".to_string(), serde_json::json!(info.toolchain));
    serde_json::Value::Object(map)
}

/// Reads `go.mod` and the toolchain; `None` means there is nothing to check.
fn gather(root: &Path) -> Result<Option<Info>, String> {
    let data = match std::fs::read(root.join("go.mod")) {
        Ok(data) => String::from_utf8_lossy(&data).to_string(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("read go.mod: {e}")),
    };
    let Some(running) = runtime_version() else {
        return Ok(None);
    };
    let mut info = Info {
        gomod_go: go_directive(&data).unwrap_or_default(),
        running_go: running,
        toolchain: std::env::var("GOTOOLCHAIN").unwrap_or_default(),
        ..Info::default()
    };
    if info.gomod_go.is_empty() {
        info.no_directive = true;
        info.ok = true;
        return Ok(Some(info));
    }
    info.ok = compare_versions(&info.running_go, &info.gomod_go) != std::cmp::Ordering::Less;
    if !info.ok && info.toolchain == "local" {
        info.warning = "GOTOOLCHAIN=local prevents automatic toolchain upgrade".to_string();
    }
    Ok(Some(info))
}

/// The human-readable body printed when `--output` is not set.
pub fn text_body(result: &GateResult) -> String {
    let mut body = String::from("🔢 Checking Go version compatibility...\n");
    let Some(extra) = result.report.extra.as_ref() else {
        if let Some(line) = crate::gates::skipped_line(&result.report.summary) {
            body.push_str(&line);
        }
        return body;
    };
    let field = |name: &str| {
        extra
            .get(name)
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string()
    };
    let gomod = field("gomod_go");
    let running = field("running_go");
    let toolchain = field("toolchain");
    if gomod.is_empty() {
        body.push_str("  No go directive in go.mod\n");
        return body;
    }
    body.push_str(&format!("  go.mod requires:  go {gomod}\n"));
    body.push_str(&format!("  Running:          go {running}\n"));
    if !toolchain.is_empty() {
        body.push_str(&format!("  GOTOOLCHAIN:      {toolchain}\n"));
    }
    if let Some(warning) = result
        .report
        .diagnostics
        .iter()
        .find(|d| d.severity == Severity::Warning)
    {
        body.push_str(&format!("  ⚠ {}\n", warning.message));
    }
    if result.passed() {
        body.push_str("✅ Go version check passed\n");
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
    fn a_newer_toolchain_satisfies_the_directive() {
        use std::cmp::Ordering;
        assert_eq!(compare_versions("1.22.2", "1.21"), Ordering::Greater);
        assert_eq!(compare_versions("1.21.0", "1.21"), Ordering::Equal);
        assert_eq!(compare_versions("1.20.14", "1.21"), Ordering::Less);
        assert_eq!(compare_versions("1.25.11", "1.25.0"), Ordering::Greater);
    }

    #[test]
    fn parses_the_first_three_numeric_components() {
        assert_eq!(parse_version("1.25.11"), [1, 25, 11]);
        assert_eq!(parse_version("1.26"), [1, 26, 0]);
        assert_eq!(parse_version("1.26.0-rc1"), [1, 26, 0]);
    }
}
