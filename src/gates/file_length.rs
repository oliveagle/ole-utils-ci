// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `file-length` gate: type-aware line budgets.
//!
//! Source and script files are held to 400/500 lines, HTML to 800/1200 and
//! declarative files (YAML, JSON, CSS, SQL) to 800/1000. A project may lower
//! a budget but never raise one, and generated files are exempt.

use crate::config::{limits, Config};
use crate::fsutil;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report, Severity};
use crate::quality_config::{QualityConfig, QualityLineLimit};

/// The line budgets and the extension-to-profile mapping.
pub mod profiles;

/// Validation of the `.devine-utils-ci.jsonc` file-length section.
pub mod config;

pub use config::validate_config;
pub use profiles::{
    base_limit, clamp, default_profiles, matching_path_rule, profile_for_extension, tighten,
    LineLimit,
};

/// One file that broke its budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLengthViolation {
    /// Path relative to the project root.
    pub path: String,
    /// The line count of the file.
    pub lines: usize,
    /// The warning threshold that applied.
    pub warn: usize,
    /// The hard threshold that applied.
    pub max: usize,
    /// The budget profile that applied.
    pub profile: String,
    /// Whether the file broke the warning or the hard threshold.
    pub severity: Severity,
}

/// Scans the project for files that break their line budget.
pub fn scan(cfg: &Config, qcfg: &QualityConfig) -> Vec<FileLengthViolation> {
    let mut violations = Vec::new();
    for entry in fsutil::walk_files(cfg.root()) {
        let ext = entry.extension();
        if ext.is_empty()
            || fsutil::is_dependency_lock_file(&entry.rel)
            || (ext == ".sql" && fsutil::is_migration_path(&entry.rel))
        {
            continue;
        }
        let Some(profile) = profile_for_extension(&ext) else {
            continue;
        };
        let Some(data) = fsutil::read_file(&entry.path) else {
            continue;
        };
        if fsutil::is_generated_file(&data) {
            continue;
        }
        let limit = resolve_limit(cfg, qcfg, &entry.rel, profile);
        if limit.max == 0 {
            continue;
        }
        let lines = fsutil::count_text_lines(&data);
        let severity = if lines > limit.max {
            Severity::Error
        } else if lines > limit.warn {
            Severity::Warning
        } else {
            continue;
        };
        violations.push(FileLengthViolation {
            path: entry.rel,
            lines,
            warn: limit.warn,
            max: limit.max,
            profile: profile.to_string(),
            severity,
        });
    }
    violations
}

/// Resolves the budget that applies to `rel`.
pub fn resolve_limit(cfg: &Config, qcfg: &QualityConfig, rel: &str, profile: &str) -> LineLimit {
    let defaults = default_profiles();
    let mut limit = defaults.get(profile).copied().unwrap_or(LineLimit {
        warn: limits::HARD_SOURCE_WARN_LINES,
        max: limits::HARD_SOURCE_MAX_LINES,
    });
    if let Some(fl) = &qcfg.file_length {
        if let Some(custom) = fl.profiles.get(profile) {
            tighten(&mut limit, custom.warn, custom.max);
        }
        if let Some(rule) = matching_path_rule(&fl.paths, rel) {
            tighten(&mut limit, rule.warn, rule.max);
        }
    }
    clamp(&mut limit, profile, cfg);
    if limit.warn > limit.max {
        limit.warn = limit.max;
    }
    limit
}

/// Runs the gate and returns its result.
pub fn run(cfg: &Config) -> Result<GateResult, String> {
    let qcfg = QualityConfig::load(cfg.root())?.unwrap_or_default();
    validate_config(&qcfg)?;
    let violations = scan(cfg, &qcfg);
    let mut report = Report::new("file-length");
    report.extra = Some(serde_json::json!({
        "max_lines": cfg.max_file_lines,
        "profiles": ["source", "script", "html", "declarative"],
    }));
    for v in &violations {
        report.add(Diagnostic {
            severity: v.severity,
            path: v.path.clone(),
            line: v.lines,
            col: 0,
            rule: format!("file-length.{}", v.profile),
            message: format!(
                "file has {} lines (warn {}, max {})",
                v.lines, v.warn, v.max
            ),
            detail: String::new(),
        });
    }
    let (errors, warnings) = count_severities(&violations);
    report.summary = format!("file-length: {errors} error(s), {warnings} warning(s)");
    if errors == 0 {
        Ok(GateResult::finish(report))
    } else {
        Ok(GateResult::failed(
            "file-length",
            report.summary.clone(),
            report,
        ))
    }
}

fn count_severities(violations: &[FileLengthViolation]) -> (usize, usize) {
    let errors = violations
        .iter()
        .filter(|v| v.severity == Severity::Error)
        .count();
    let warnings = violations
        .iter()
        .filter(|v| v.severity == Severity::Warning)
        .count();
    (errors, warnings)
}

/// The human-readable body printed when `--output` is not set.
pub fn text_body(result: &GateResult) -> String {
    let mut body = String::from("📏 Checking type-aware file length budgets...\n");
    let errors = result
        .report
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warnings = result.report.diagnostics.len() - errors;
    for d in &result.report.diagnostics {
        let mark = if d.severity == Severity::Error {
            "✗"
        } else {
            "⚠"
        };
        body.push_str(&format!("  {mark} {} — {}\n", d.path, d.message));
    }
    if errors > 0 {
        body.push_str(&format!(
            "✗ FILE LENGTH VIOLATION: {errors} file(s) exceed their hard limit.\n"
        ));
        body.push_str("  Split the file by responsibility; do not raise the source/script cap.\n");
    } else if warnings > 0 {
        body.push_str(&format!(
            "⚠ File length check passed with {warnings} warning(s); reduce these before they reach the hard limit.\n"
        ));
    } else {
        body.push_str("✅ File length check passed\n");
    }
    body
}

/// The default budget of a profile, for callers that only need the number.
pub fn profile_budget(profile: &str) -> QualityLineLimit {
    let defaults = default_profiles();
    let limit = defaults.get(profile).copied().unwrap_or(LineLimit {
        warn: limits::HARD_SOURCE_WARN_LINES,
        max: limits::HARD_SOURCE_MAX_LINES,
    });
    QualityLineLimit {
        warn: limit.warn,
        max: limit.max,
    }
}

#[cfg(test)]
mod tests;
