// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `ci-lint` command: validate the GitLab CI pipeline file.
//!
//! The file has to parse as YAML, and three text-level mistakes that GitLab
//! rejects are reported as well: a tab character, trailing whitespace and an
//! unquoted colon inside a list item.

use std::path::Path;

use crate::config::Config;
use crate::output::{Diagnostic, Report};

/// One problem found in the pipeline file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiLintIssue {
    /// 1-based line number.
    pub line: usize,
    /// The rule that was broken.
    pub rule: String,
    /// A one-line description.
    pub message: String,
}

/// Scans the pipeline file for the text-level mistakes GitLab rejects.
pub fn scan_text(text: &str) -> Vec<CiLintIssue> {
    let mut issues = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        if line.contains('\t') {
            issues.push(CiLintIssue {
                line: number,
                rule: "tab-char".to_string(),
                message: "tab character detected (use spaces)".to_string(),
            });
        }
        if line.ends_with(' ') {
            issues.push(CiLintIssue {
                line: number,
                rule: "trailing-whitespace".to_string(),
                message: "trailing whitespace".to_string(),
            });
        }
        if let Some(content) = list_item_content(line) {
            if let Some(issue) = unquoted_colon(content) {
                issues.push(CiLintIssue {
                    line: number,
                    rule: "unquoted-colon".to_string(),
                    message: issue,
                });
            }
        }
    }
    issues
}

/// The text of a YAML list item, when the line holds one.
fn list_item_content(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    trimmed.strip_prefix("- ")
}

/// Checks a list item for an unquoted colon in a scalar value.
fn unquoted_colon(content: &str) -> Option<String> {
    if content.contains('"') || content.contains('\'') {
        return None;
    }
    let (key, _) = content.split_once(": ")?;
    if is_simple_key(key) {
        return None;
    }
    Some(format!(
        "unquoted colon in string: {:?} (wrap in quotes)",
        content
    ))
}

/// Whether a key looks like a plain mapping key rather than a scalar.
fn is_simple_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/')
}

/// The `ci-lint` command.
pub fn ci_lint(cfg: &Config) -> Result<i32, String> {
    ci_lint_file(cfg, Path::new(".gitlab-ci.yml"))
}

/// The `ci-lint` command against one specific pipeline file.
pub fn ci_lint_file(cfg: &Config, relative: &Path) -> Result<i32, String> {
    let path = cfg.root().join(relative);
    let display = relative.display().to_string();
    let Ok(raw) = std::fs::read_to_string(&path) else {
        let mut report = Report::new("ci-lint");
        report.summary = format!("ci-lint: no {display}, skipped");
        print!("{}", report.render_text());
        return Ok(0);
    };
    let mut report = Report::new("ci-lint");
    match serde_yaml::from_str::<serde_yaml::Value>(&raw) {
        Ok(_) => {}
        Err(e) => {
            report.add(Diagnostic::error(
                display.clone(),
                "yaml-syntax",
                e.to_string(),
            ));
            report.summary = "ci-lint: YAML syntax error".to_string();
            print!("{}", report.render_text());
            return Err(report.summary);
        }
    }
    let issues = scan_text(&raw);
    for issue in &issues {
        report.add(
            Diagnostic::error(display.clone(), issue.rule.clone(), issue.message.clone())
                .at_line(issue.line),
        );
    }
    report.extra = Some(serde_json::json!({ "issues": issues.len() }));
    if report.ok {
        report.summary = "ci-lint: the pipeline file is clean".to_string();
        print!("{}", report.render_text());
        Ok(0)
    } else {
        report.summary = format!("ci-lint: {} issue(s) found", issues.len());
        print!("{}", report.render_text());
        Err(report.summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_tabs_and_trailing_whitespace() {
        let issues = scan_text("job:\n\tscript: echo hi\n");
        assert!(issues.iter().any(|i| i.rule == "tab-char"));
        let issues = scan_text("job:  \n");
        assert!(issues.iter().any(|i| i.rule == "trailing-whitespace"));
    }

    #[test]
    fn reports_an_unquoted_colon() {
        let issues = scan_text("  - echo a: b\n");
        assert!(issues.iter().any(|i| i.rule == "unquoted-colon"));
        let issues = scan_text("  - \"echo a: b\"\n");
        assert!(!issues.iter().any(|i| i.rule == "unquoted-colon"));
    }
}
