// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `ci-lint` command: validate the GitLab CI pipeline file.
//!
//! The file has to parse as YAML. Three text-level mistakes GitLab rejects are
//! reported — a tab character, trailing whitespace and an unquoted colon
//! inside a list item — and then every top-level job is checked for the shape
//! GitLab needs: a job with no `script:`, `trigger:` or `extends:`, a script
//! whose entries are not strings, and a `>` folded block scalar in a script
//! section.

use std::path::Path;

use serde_norway::Value;

use crate::config::Config;
use crate::output::{Diagnostic, Report};

mod scan;

pub use scan::{scan_folded, scan_jobs, scan_text, CiLintIssue};

/// The `ci-lint` command.
pub fn ci_lint(cfg: &Config) -> Result<i32, String> {
    ci_lint_file(cfg, Path::new(".gitlab-ci.yml"))
}

/// The `ci-lint` command against one specific pipeline file.
pub fn ci_lint_file(cfg: &Config, relative: &Path) -> Result<i32, String> {
    let path = cfg.root().join(relative);
    let display = relative.display().to_string();
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Err(format!("{display} not found in {}", cfg.root().display()));
    };
    let mut report = Report::new("ci-lint");
    let parsed = match serde_norway::from_str::<Value>(&raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            report.add(Diagnostic::error(
                display.clone(),
                "yaml-syntax",
                error.to_string(),
            ));
            report.summary = "ci-lint: YAML syntax error".to_string();
            if cfg.output.is_empty() {
                return Err("YAML syntax error".to_string());
            }
            print!("{}", report.render(crate::output::parse_mode(&cfg.output)));
            return Err(report.summary);
        }
    };
    let mut issues = scan_text(&raw);
    issues.extend(scan_jobs(&parsed));
    issues.extend(scan_folded(&raw));
    for issue in &issues {
        report.add(
            Diagnostic::error(display.clone(), issue.rule.clone(), issue.message.clone())
                .at_line(issue.line),
        );
    }
    if cfg.output.is_empty() {
        println!("🔍 Validating CI configuration...");
        if issues.is_empty() {
            println!("✅ CI configuration is valid");
            return Ok(0);
        }
        let lines: Vec<String> = issues
            .iter()
            .map(|issue| {
                if issue.line > 0 {
                    format!("  Line {}: {}", issue.line, issue.message)
                } else {
                    format!("  {}", issue.message)
                }
            })
            .collect();
        return Err(format!(
            "CI lint found {} issue(s):\n{}",
            issues.len(),
            lines.join("\n")
        ));
    }
    if issues.is_empty() {
        report.summary = "ci-lint: .gitlab-ci.yml passes local static checks".to_string();
    } else {
        report.summary = format!("ci-lint: {} issue(s) in .gitlab-ci.yml", issues.len());
    }
    print!("{}", report.render(crate::output::parse_mode(&cfg.output)));
    if report.ok {
        Ok(0)
    } else {
        Err(format!("{} CI lint issue(s)", issues.len()))
    }
}

#[cfg(test)]
mod tests;
