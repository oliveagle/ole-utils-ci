// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `bugs` gate: scans source files for `BUG` / `FIXME` / `HACK` / `XXX`. //nolint:bugs
//!
//! The markers are matched on word boundaries so that a variable named
//! `debugCount` does not trip the scan, and a line carrying a `//nolint`
//! comment is skipped — useful for the scanner's own test fixtures.

use std::collections::BTreeSet;

use regex::Regex;

use crate::config::Config;
use crate::fsutil::{self, Entry};
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};
use crate::static_regex;

/// The markers the gate looks for.
pub const MARKERS: [&str; 4] = ["BUG", "FIXME", "HACK", "XXX"]; //nolint:bugs

/// Extensions the gate scans.
const SCANNED_EXTENSIONS: [&str; 2] = [".go", ".rs"];

/// One marker found in a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BugFinding {
    /// Path relative to the project root.
    pub path: String,
    /// 1-based line number.
    pub line: usize,
    /// The marker that was found.
    pub marker: String,
}

/// Builds the regex that matches a marker on a word boundary.
fn marker_regex() -> Regex {
    let pattern = format!(r"\b({})\b", MARKERS.join("|"));
    static_regex::compile(&pattern)
}

/// Whether the line carries a `nolint` comment that skips this gate.
pub fn has_nolint_skip(line: &str) -> bool {
    let Some(idx) = line.find("//") else {
        return false;
    };
    let comment = &line[idx..];
    comment.contains("nolint:bugs")
        || comment.contains("nolint:bug")
        || comment.contains("//nolint")
}

/// Scans the project for bug markers.
pub fn scan(
    project_root: &std::path::Path,
    qcfg: &crate::quality_config::QualityConfig,
) -> (Vec<BugFinding>, usize) {
    let re = marker_regex();
    let mut findings = Vec::new();
    let mut total_files = 0usize;
    for entry in fsutil::walk_files(project_root) {
        if !SCANNED_EXTENSIONS.contains(&entry.extension().as_str()) {
            continue;
        }
        if qcfg.should_ignore(&entry.rel, "bugs") {
            continue;
        }
        total_files += 1;
        let Some(data) = fsutil::read_file(&entry.path) else {
            continue;
        };
        let text = String::from_utf8_lossy(&data);
        for (idx, line) in text.lines().enumerate() {
            if has_nolint_skip(line) {
                continue;
            }
            if let Some(m) = re.captures(line) {
                findings.push(BugFinding {
                    path: entry.rel.clone(),
                    line: idx + 1,
                    marker: m.get(1).map(|g| g.as_str().to_string()).unwrap_or_default(),
                });
            }
        }
    }
    (findings, total_files)
}

/// Runs the gate and returns its result.
pub fn run(cfg: &Config) -> Result<GateResult, String> {
    let qcfg = crate::quality_config::QualityConfig::load(cfg.root())?.unwrap_or_default();
    let (findings, total_files) = scan(cfg.root(), &qcfg);
    let mut report = Report::new("bugs");
    report.extra = Some(serde_json::json!({
        "files_scanned": total_files,
        "markers": MARKERS,
    }));
    for f in &findings {
        report.add(
            Diagnostic::error(
                f.path.clone(),
                f.marker.clone(),
                format!("bug marker present: {}", f.marker),
            )
            .at_line(f.line),
        );
    }
    if report.ok {
        report.summary = format!("bugs: scanned {total_files} files, no markers");
        Ok(GateResult::finish(report))
    } else {
        report.summary = format!(
            "bugs: {} marker(s) found in {total_files} files",
            findings.len()
        );
        Ok(GateResult::failed("bugs", report.summary.clone(), report))
    }
}

/// The human-readable body printed when `--output` is not set.
pub fn text_body(result: &GateResult, total_files: usize) -> String {
    let mut body = String::from("🐛 Scanning for bug markers...\n");
    if result.passed() {
        body.push_str(&format!(
            "  ✓ Scanned {total_files} files, no bug markers found\n"
        ));
        body.push_str("✅ Bug scan passed\n");
        return body;
    }
    body.push_str(&format!(
        "✗ {} MARKER VIOLATION: {} marker(s) found in {total_files} files.\n",
        MARKERS[0],
        result.report.diagnostics.len()
    ));
    for d in &result.report.diagnostics {
        body.push_str(&format!("  {}:{}: found {}\n", d.path, d.line, d.rule));
    }
    body.push('\n');
    body.push_str("═══ REQUIRED REMEDIATION (MANDATORY - DO NOT BYPASS) ═══\n");
    body.push_str(&format!(
        "  {} markers MUST be resolved immediately:\n",
        MARKERS.join("/")
    ));
    body.push_str("    - If it's a real bug → fix the code.\n");
    body.push_str("    - If it's debt → create a task/issue and remove the marker.\n");
    body.push_str("    - If it's obsolete → remove the marker.\n");
    body.push_str("  ⚠ Leaving markers as 'documentation' is NOT acceptable.\n");
    body.push_str("    Either fix it or remove the comment entirely.\n");
    body.push_str("═══════════════════════════════════════════════════════════════\n");
    body
}

/// The set of markers this gate understands.
pub fn marker_set() -> BTreeSet<&'static str> {
    MARKERS.into_iter().collect()
}

/// Whether the entry is one the gate scans.
pub fn is_scanned(entry: &Entry) -> bool {
    SCANNED_EXTENSIONS.contains(&entry.extension().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_nolint_skips() {
        assert!(has_nolint_skip("code //nolint:bugs"));
        assert!(has_nolint_skip("code //nolint:bug"));
        assert!(has_nolint_skip("code //nolint"));
        assert!(!has_nolint_skip("code // no lint here"));
    }

    #[test]
    fn marker_regex_matches_whole_words_only() {
        let re = marker_regex();
        assert!(re.is_match(&format!("{}: the thing broke", MARKERS[0])));
        assert!(!re.is_match("debugCount += 1"));
        assert!(!re.is_match("xxxLowPriority"));
    }
}
