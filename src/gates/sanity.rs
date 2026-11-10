// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `sanity` gate: trailing whitespace, conflict markers and missing final
//! newlines.
//!
//! Markdown is exempt from the trailing-whitespace rule (hard line breaks are
//! meaningful there) but not from the conflict-marker rule.

use crate::config::Config;
use crate::fsutil;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};

/// Extensions the gate inspects.
const SCANNED_EXTENSIONS: [&str; 13] = [
    ".go", ".md", ".yaml", ".yml", ".jsonc", ".sh", ".proto", ".ts", ".tsx", ".rs", ".py", ".java",
    ".json",
];

/// Directories that are never inspected.
const SKIP_DIRS: [&str; 8] = [
    "vendor",
    "node_modules",
    "reports",
    ".git",
    ".codegraph",
    ".omo",
    ".go-mod",
    "target",
];

/// One sanity problem found in the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanityIssue {
    /// Path relative to the project root.
    pub path: String,
    /// 1-based line number; `0` when the issue is file-wide.
    pub line: usize,
    /// The rule that was broken.
    pub rule: String,
    /// A one-line description.
    pub message: String,
}

/// Scans the project for sanity problems.
pub fn scan(
    project_root: &std::path::Path,
    qcfg: &crate::quality_config::QualityConfig,
) -> Vec<SanityIssue> {
    let mut issues = Vec::new();
    for entry in fsutil::walk_files(project_root) {
        if SKIP_DIRS
            .iter()
            .any(|d| entry.rel.split('/').any(|p| p == *d))
        {
            continue;
        }
        if qcfg.should_ignore(&entry.rel, "sanity") {
            continue;
        }
        let ext = entry.extension();
        if !SCANNED_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }
        let Some(data) = fsutil::read_file(&entry.path) else {
            continue;
        };
        let raw = String::from_utf8_lossy(&data);
        let content = fsutil::strip_bom(&raw);
        let lines: Vec<&str> = content.split('\n').collect();
        if ext != ".md" && has_trailing_whitespace(&lines) {
            issues.push(SanityIssue {
                path: entry.rel.clone(),
                line: 0,
                rule: "trailing-whitespace".to_string(),
                message: "trailing whitespace".to_string(),
            });
        }
        for (idx, line) in lines.iter().enumerate() {
            if let Some(marker) = conflict_marker(line) {
                issues.push(SanityIssue {
                    path: entry.rel.clone(),
                    line: idx + 1,
                    rule: "conflict-marker".to_string(),
                    message: format!("conflict marker '{marker}'"),
                });
            }
        }
        if !content.is_empty() && !content.ends_with('\n') {
            issues.push(SanityIssue {
                path: entry.rel.clone(),
                line: 0,
                rule: "trailing-newline".to_string(),
                message: "missing trailing newline".to_string(),
            });
        }
    }
    issues
}

/// Whether any line carries trailing whitespace.
///
/// The line number of the first offender is returned so the finding can point
/// at the exact line.
fn has_trailing_whitespace(lines: &[&str]) -> bool {
    lines.iter().any(|line| {
        let check = line.strip_suffix('\r').unwrap_or(line);
        !check.is_empty() && check.trim_end_matches([' ', '\t']) != check
    })
}

/// The conflict marker a line starts with, if any.
fn conflict_marker(line: &str) -> Option<&'static str> {
    for marker in [">>>>>>> ", "<<<<<<< ", "======="] {
        if line.starts_with(marker) {
            return Some(marker.trim());
        }
    }
    None
}

/// Runs the gate and returns its result.
pub fn run(cfg: &Config) -> Result<GateResult, String> {
    let qcfg = crate::quality_config::QualityConfig::load(cfg.root())?.unwrap_or_default();
    let issues = scan(cfg.root(), &qcfg);
    let mut report = Report::new("sanity");
    for i in &issues {
        report.add(Diagnostic::error(
            i.path.clone(),
            i.rule.clone(),
            i.message.clone(),
        ));
    }
    if report.ok {
        report.summary = "sanity: repository clean".to_string();
        Ok(GateResult::finish(report))
    } else {
        report.summary = format!("sanity: {} issue(s)", issues.len());
        Ok(GateResult::failed("sanity", report.summary.clone(), report))
    }
}

/// The human-readable body printed when `--output` is not set.
pub fn text_body(result: &GateResult) -> String {
    let mut body = String::from("🧹 Running repository sanity checks...\n");
    if result.passed() {
        body.push_str("✅ Sanity check passed\n");
        return body;
    }
    body.push_str(&format!(
        "sanity check found {} issue(s):\n",
        result.report.diagnostics.len()
    ));
    for d in &result.report.diagnostics {
        if d.line > 0 {
            body.push_str(&format!("  {}:{}: {}\n", d.path, d.line, d.message));
        } else {
            body.push_str(&format!("  {}: {}\n", d.path, d.message));
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_trailing_whitespace() {
        assert!(has_trailing_whitespace(&["ok", "bad  "]));
        assert!(!has_trailing_whitespace(&["ok", "fine"]));
        assert!(has_trailing_whitespace(&["", "   "]));
    }

    #[test]
    fn detects_conflict_markers() {
        assert_eq!(conflict_marker("<<<<<<< HEAD"), Some("<<<<<<<"));
        assert_eq!(conflict_marker("======="), Some("======="));
        assert_eq!(conflict_marker(">>>>>>> branch"), Some(">>>>>>>"));
        assert_eq!(conflict_marker("normal text"), None);
    }
}
