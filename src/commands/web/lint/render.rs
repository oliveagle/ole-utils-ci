// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `web_lint` result envelope and its three renderings.

use super::super::engine::{self, Finding};
use super::run::{Failure, Outcome};
use crate::output::render::encode_value;
use crate::output::Mode;

/// The result object for one linted file.
pub fn result(rel: &str, language: &str, findings: Vec<Finding>) -> serde_json::Value {
    let issues: Vec<serde_json::Value> = findings
        .iter()
        .map(|finding| {
            serde_json::json!({
                "line": finding.line,
                "col": finding.col,
                "severity": finding.severity,
                "rule": finding.rule,
                "message": finding.message,
            })
        })
        .collect();
    serde_json::json!({
        "file": rel,
        "language": language,
        "engine": engine::engine_of(language),
        "issues": issues,
    })
}

/// The result object for a file that could not be read.
pub fn read_failure(rel: &str, language: &str, error: &std::io::Error) -> serde_json::Value {
    serde_json::json!({
        "file": rel,
        "language": language,
        "engine": "-",
        "issues": serde_json::Value::Array(Vec::new()),
        "skipped": format!("read {rel}: {error}"),
    })
}

/// Builds the `web_lint` result envelope.
///
/// The keys are written in the order Go's `encoding/json` produces for the
/// map it builds: alphabetical at every level, and struct-field order inside
/// each file result.
pub fn envelope(results: &[Outcome]) -> serde_json::Value {
    let mut files = 0usize;
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut infos = 0usize;
    let mut skip_count = 0usize;
    // Go marshals the `engines` map with sorted keys.
    let mut engines = std::collections::BTreeMap::new();
    for (_, result) in results {
        if result.get("skipped").is_some() {
            skip_count += 1;
            continue;
        }
        files += 1;
        if let (Some(language), Some(engine)) = (
            result.get("language").and_then(|v| v.as_str()),
            result.get("engine").and_then(|v| v.as_str()),
        ) {
            if engine != "-" {
                engines.insert(language.to_string(), engine.to_string());
            }
        }
        for issue in array(result, "issues") {
            match issue.get("severity").and_then(|v| v.as_str()) {
                Some("error") => errors += 1,
                Some("warning") => warnings += 1,
                _ => infos += 1,
            }
        }
    }
    serde_json::json!({
        "notes": serde_json::Value::Array(Vec::new()),
        "ok": errors == 0,
        "results": results
            .iter()
            .map(|(_, result)| result.clone())
            .collect::<Vec<_>>(),
        "summary": {
            "engines": engines,
            "errors": errors,
            "files": files,
            "infos": infos,
            "skipped": skip_count,
            "warnings": warnings,
        },
    })
}

/// Whether the findings reach the `--fail-on` severity.
pub fn fails(envelope: &serde_json::Value, fail_on: &str) -> bool {
    let errors = summary(envelope, "errors");
    let warnings = summary(envelope, "warnings");
    match fail_on.to_ascii_lowercase().as_str() {
        "none" => false,
        "warning" => errors > 0 || warnings > 0,
        _ => errors > 0,
    }
}

/// Writes the envelope in the requested mode.
pub fn write(mode: Mode, envelope: &serde_json::Value) -> Result<(), Failure> {
    if mode == Mode::Text {
        print!("{}", text(envelope));
        return Ok(());
    }
    let rendered = encode_value(mode.as_str(), false, envelope)
        .map_err(|error| Failure::Io(error.to_string()))?;
    print!("{rendered}");
    Ok(())
}

/// The text rendering of the envelope, line for line as Go writes it.
pub fn text(envelope: &serde_json::Value) -> String {
    let mut out = String::new();
    for result in array(envelope, "results") {
        if let Some(skipped) = result.get("skipped").and_then(|v| v.as_str()) {
            out.push_str(&format!("  skip  {}: {skipped}\n", string(result, "file")));
            continue;
        }
        for issue in array(result, "issues") {
            out.push_str(&format!(
                "{}:{}:{}  {}  {}  {}\n",
                string(result, "file"),
                number(issue, "line"),
                number(issue, "col"),
                string(issue, "severity"),
                string(issue, "rule"),
                string(issue, "message"),
            ));
        }
    }
    out.push_str(&format!(
        "summary: {} file(s), {} error(s), {} warning(s), {} skipped\n",
        summary(envelope, "files"),
        summary(envelope, "errors"),
        summary(envelope, "warnings"),
        summary(envelope, "skipped"),
    ));
    for note in array(envelope, "notes") {
        if let Some(note) = note.as_str() {
            out.push_str(&format!("note: {note}\n"));
        }
    }
    out
}

/// The array under `key`, or an empty one.
fn array<'a>(value: &'a serde_json::Value, key: &str) -> &'a [serde_json::Value] {
    value
        .get(key)
        .and_then(|v| v.as_array())
        .map(|v| v.as_slice())
        .unwrap_or(&[])
}

/// The string under `key`, or `""`.
fn string(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// The number under `key`, or `0`.
fn number(value: &serde_json::Value, key: &str) -> u64 {
    value.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

/// One counter of the summary object.
pub fn summary(envelope: &serde_json::Value, key: &str) -> u64 {
    envelope
        .get("summary")
        .map(|summary| number(summary, key))
        .unwrap_or(0)
}
