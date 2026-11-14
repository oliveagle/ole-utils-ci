// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The text and job-shape scanners behind `ci-lint`.

use serde_norway::Value;

/// Top-level keys that are not job definitions.
const NOT_A_JOB: [&str; 9] = [
    "stages",
    "variables",
    "cache",
    "default",
    "workflow",
    "include",
    "image",
    "before_script",
    "after_script",
];

/// The script-shaped keys of one job.
const SCRIPT_KEYS: [&str; 3] = ["script", "before_script", "after_script"];

/// One problem found in the pipeline file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiLintIssue {
    /// 1-based line number; 0 when the issue is not line-specific.
    pub line: usize,
    /// The rule that was broken.
    pub rule: String,
    /// A one-line description.
    pub message: String,
}

impl CiLintIssue {
    /// One issue without a line, for the shape checks.
    fn at_rule(rule: &str, message: impl Into<String>) -> Self {
        Self {
            line: 0,
            rule: rule.to_string(),
            message: message.into(),
        }
    }
}

/// Scans the pipeline file for the text-level mistakes GitLab rejects.
pub fn scan_text(text: &str) -> Vec<CiLintIssue> {
    let mut issues = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        if let Some(content) = list_item_content(line) {
            if let Some(issue) = unquoted_colon(content) {
                issues.push(CiLintIssue {
                    line: number,
                    rule: "unquoted-colon".to_string(),
                    message: issue,
                });
            }
        }
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
        && !key
            .chars()
            .any(|c| matches!(c, ' ' | '"' | '\'' | '{' | '}' | '[' | ']'))
}

/// Checks every top-level job of the parsed pipeline for the shape GitLab
/// needs: `script:` / `trigger:` / `extends:`, string script entries.
pub fn scan_jobs(parsed: &Value) -> Vec<CiLintIssue> {
    let Some(jobs) = parsed.as_mapping() else {
        return Vec::new();
    };
    let mut issues = Vec::new();
    for (key, value) in jobs {
        let Some(name) = key.as_str() else {
            continue;
        };
        if NOT_A_JOB.contains(&name) {
            continue;
        }
        let Some(job) = value.as_mapping() else {
            // A hidden template (`.name`) may legally have no `script:`; a
            // job that is empty or is not a mapping is still a problem.
            if !name.starts_with('.') {
                issues.push(CiLintIssue::at_rule(
                    "empty-job",
                    format!("Job {name:?} is empty — every job must have 'script:' or 'trigger:'"),
                ));
            }
            continue;
        };
        // Templates exist to be extended, so they skip the shape check.
        if !name.starts_with('.') && !has(job, &["script", "trigger", "extends"]) {
            issues.push(CiLintIssue::at_rule(
                "no-script",
                format!(
                    "Job {name:?} has no 'script:', 'trigger:', or 'extends:' — GitLab will reject it"
                ),
            ));
        }
        for script_key in SCRIPT_KEYS {
            if let Some(raw) = job.get(script_key) {
                check_script_entries(&mut issues, name, script_key, raw);
            }
        }
    }
    issues
}

/// Whether a mapping has at least one of the named keys.
fn has(job: &serde_norway::Mapping, names: &[&str]) -> bool {
    names.iter().any(|name| job.get(*name).is_some())
}

/// Reports the first non-string leaf of a script section, recursing into the
/// nested arrays GitLab allows.
fn check_script_entries(issues: &mut Vec<CiLintIssue>, job: &str, key: &str, value: &Value) {
    if let Some(list) = value.as_sequence() {
        for (index, item) in list.iter().enumerate() {
            if item.as_str().is_some() {
                continue;
            }
            if item.is_sequence() {
                check_script_entries(issues, job, &format!("{key}[{index}]"), item);
                continue;
            }
            issues.push(CiLintIssue::at_rule(
                "script-not-string",
                format!(
                    "Job {job:?} {key}[{index}]: not a string (YAML parsed as {}) — wrap in quotes",
                    yaml_type(item)
                ),
            ));
        }
        return;
    }
    if value.as_str().is_some() {
        return;
    }
    issues.push(CiLintIssue::at_rule(
        "script-not-string",
        format!(
            "Job {job:?} {key}: not a string (YAML parsed as {}) — wrap in quotes",
            yaml_type(value)
        ),
    ));
}

/// The type name the error message prints for a non-string YAML value.
fn yaml_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "string",
        Value::Bool(_) => "bool",
        Value::Number(_) => "int",
        Value::String(_) => "string",
        Value::Sequence(_) => "[]interface {}",
        Value::Mapping(_) => "map[string]interface {}",
        Value::Tagged(_) => "interface {}",
    }
}

/// Reports a `>` folded block scalar in a script section, where folding
/// turns newlines into spaces and can split multi-line arguments.
pub fn scan_folded(text: &str) -> Vec<CiLintIssue> {
    let lines: Vec<&str> = text.lines().collect();
    let mut issues = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !(trimmed.ends_with('>') || trimmed.ends_with(">-") || trimmed.ends_with(">+")) {
            continue;
        }
        for previous in lines[..index].iter().rev().take(10) {
            let previous = previous.trim();
            if previous.is_empty() {
                continue;
            }
            if SCRIPT_KEYS
                .iter()
                .any(|key| previous.contains(&format!("{key}:")))
            {
                issues.push(CiLintIssue {
                    line: index + 1,
                    rule: "folded-block-scalar".to_string(),
                    message: "YAML > (folded block scalar) in script: section — folding converts newlines to spaces and can split multi-line command arguments. Use a single-line string or | (literal block scalar) instead.".to_string(),
                });
                return issues;
            }
        }
    }
    issues
}
