// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `complexity` gate: function-level cyclomatic complexity.
//!
//! **A budget is a requirement, not advice.** A function over the budget fails
//! the gate rather than producing a warning that leaves the exit code at zero.
//! Rust is covered alongside Go, Tengo, TypeScript and JavaScript.

pub mod lang;
pub mod scanners;

use crate::config::{limits, Config};
use crate::fsutil;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report, Severity};
use crate::quality_config::QualityConfig;

pub use lang::FunctionMetric;

/// One function that broke its complexity budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexityViolation {
    /// The metric that was measured.
    pub metric: FunctionMetric,
    /// Whether the function broke the warning or the hard threshold.
    pub severity: Severity,
}

/// The thresholds that apply to a run.
#[derive(Debug, Clone, Copy)]
pub struct Thresholds {
    /// The warning threshold.
    pub warn: usize,
    /// The hard threshold.
    pub max: usize,
}

/// Resolves the effective thresholds for a run.
pub fn effective_thresholds(cfg: &Config, qcfg: &QualityConfig) -> Thresholds {
    let mut warn = cfg.complexity_warn;
    let mut max = cfg.max_complexity;
    if warn == 0 || warn > limits::HARD_COMPLEXITY_WARN {
        warn = limits::HARD_COMPLEXITY_WARN;
    }
    if max == 0 || max > limits::HARD_COMPLEXITY_MAX {
        max = limits::HARD_COMPLEXITY_MAX;
    }
    if let Some(c) = &qcfg.complexity {
        if c.max > 0 && c.max < max {
            max = c.max;
        }
        if c.warn > 0 && c.warn < warn {
            warn = c.warn;
        }
    }
    if warn > max {
        warn = max;
    }
    Thresholds { warn, max }
}

/// Scans the project and returns the total function count and the violations.
pub fn scan(root: &std::path::Path, thresholds: Thresholds) -> (usize, Vec<ComplexityViolation>) {
    let mut total = 0usize;
    let mut violations = Vec::new();
    for entry in fsutil::walk_files(root) {
        let Some(language) = lang::language_for_extension(&entry.extension()) else {
            continue;
        };
        let Some(data) = fsutil::read_file(&entry.path) else {
            continue;
        };
        let source = String::from_utf8_lossy(&data);
        let mut metrics = lang::scan_file(&entry.rel, &source, language);
        metrics.sort_by(|a, b| a.line.cmp(&b.line).then(a.function.cmp(&b.function)));
        for m in metrics {
            total += 1;
            if let Some(severity) = severity_for(m.complexity, thresholds) {
                violations.push(ComplexityViolation {
                    metric: m,
                    severity,
                });
            }
        }
    }
    (total, violations)
}

/// A function over the budget is a failure, whichever tier it broke.
fn severity_for(value: usize, thresholds: Thresholds) -> Option<Severity> {
    if value > thresholds.warn {
        Some(Severity::Error)
    } else {
        None
    }
}

/// The languages the report names.
///
/// The Go original names its four scanners. This port also understands Rust,
/// so a Cargo project gets an extra entry — and a Go project's payload stays
/// byte-identical to the original's.
fn reported_languages(root: &std::path::Path) -> Vec<&'static str> {
    let mut languages = lang::LANGUAGES.to_vec();
    if root.join("Cargo.toml").is_file() {
        languages.push(lang::RUST_LANGUAGE);
    }
    languages
}

/// Runs the gate and returns its result.
pub fn run(cfg: &Config) -> Result<GateResult, String> {
    let qcfg = QualityConfig::load(cfg.root())?.unwrap_or_default();
    validate_config(&qcfg)?;
    let thresholds = effective_thresholds(cfg, &qcfg);
    let (total, violations) = scan(cfg.root(), thresholds);
    let mut report = Report::new("complexity");
    // The keys are written in the sorted order Go's `map[string]any` encodes
    // them in, so the JSON payload is byte-identical to the original's.
    report.extra = Some(serde_json::json!({
        "functions_scanned": total,
        "languages": reported_languages(cfg.root()),
        "max_threshold": thresholds.max,
        "warn_threshold": thresholds.warn,
    }));
    for v in &violations {
        let mut diagnostic = Diagnostic::error(
            v.metric.path.clone(),
            format!("complexity.{}", v.metric.language),
            format!(
                "{} has complexity {} (budget {}, cap {})",
                v.metric.function, v.metric.complexity, thresholds.warn, thresholds.max
            ),
        )
        .at_line(v.metric.line);
        diagnostic.severity = v.severity;
        report.add(diagnostic);
    }
    let errors = violations
        .iter()
        .filter(|v| v.severity == Severity::Error)
        .count();
    let warnings = violations.len() - errors;
    report.summary =
        format!("complexity: {total} function(s), {errors} error(s), {warnings} warning(s)");
    if errors == 0 {
        Ok(GateResult::finish(report))
    } else {
        let message = format!("{errors} function(s) exceed complexity threshold");
        Ok(
            GateResult::failed("complexity", report.summary.clone(), report)
                .failing(message.clone(), message),
        )
    }
}

/// Rejects a config that tries to raise the built-in budget.
pub fn validate_config(qcfg: &QualityConfig) -> Result<(), String> {
    let section = &qcfg.ignore.complexity;
    if let Some(section) = section {
        if !section.paths.is_empty() {
            return Err(
                "complexity: path ignores are not allowed for hard quality budgets; lower the budget instead"
                    .to_string(),
            );
        }
    }
    let Some(c) = &qcfg.complexity else {
        return Ok(());
    };
    if c.warn > limits::HARD_COMPLEXITY_WARN || c.max > limits::HARD_COMPLEXITY_MAX {
        return Err(format!(
            "complexity: config can only tighten built-in warn={} max={}",
            limits::HARD_COMPLEXITY_WARN,
            limits::HARD_COMPLEXITY_MAX
        ));
    }
    if c.warn > 0 && c.max > 0 && c.warn > c.max {
        return Err(format!(
            "complexity: warn={} cannot exceed max={}",
            c.warn, c.max
        ));
    }
    Ok(())
}

/// The human-readable body printed when `--output` is not set.
pub fn text_body(result: &GateResult, total: usize, thresholds: Thresholds) -> String {
    let mut body = format!(
        "🌀 Checking cyclomatic complexity (budget: {}, cap: {})...\n",
        thresholds.warn, thresholds.max
    );
    let errors = result
        .report
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    for d in &result.report.diagnostics {
        let mark = if d.severity == Severity::Error {
            "✗"
        } else {
            "⚠"
        };
        body.push_str(&format!("  {mark} {}:{} — {}\n", d.path, d.line, d.message));
    }
    if errors > 0 {
        body.push_str(&format!(
            "✗ COMPLEXITY VIOLATION: {errors} function(s) exceed the budget of {} — a budget is a requirement.\n",
            thresholds.warn
        ));
        body.push_str("  Extract helpers, reduce nesting, or simplify boolean conditions.\n");
    } else {
        body.push_str(&format!(
            "  Checked {total} functions, all within the budget of {}\n",
            thresholds.warn
        ));
        body.push_str("✅ Complexity check passed\n");
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_function_over_its_budget_is_an_error_not_a_warning() {
        // 18 sits inside the 15/20 band: once a warning, now a failure.
        let thresholds = Thresholds { warn: 15, max: 20 };
        assert_eq!(
            severity_for(18, thresholds),
            Some(Severity::Error),
            "over budget must fail the gate, not warn about it"
        );
        assert_eq!(
            severity_for(15, thresholds),
            None,
            "the budget itself passes"
        );
        assert_eq!(
            severity_for(21, thresholds),
            Some(Severity::Error),
            "over the cap still fails"
        );
    }

    #[test]
    fn counts_decision_points() {
        assert_eq!(lang::count_decisions("{}"), 1);
        assert_eq!(lang::count_decisions("if a { b() }"), 2);
        assert_eq!(lang::count_decisions("if a && b { }"), 3);
        assert_eq!(lang::count_decisions("for x in y { }"), 2);
    }

    #[test]
    fn finds_rust_functions() {
        let source = "fn simple() {}\nfn branchy(a: bool) {\n  if a { }\n}\n";
        let metrics = scanners::scan_rust("a.rs", source);
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[0].function, "simple");
        assert_eq!(metrics[1].function, "branchy");
        assert_eq!(metrics[1].complexity, 2);
    }

    #[test]
    fn finds_go_functions() {
        let source = "func simple() {}\nfunc (r *T) branchy(a bool) {\n\tif a {}\n}\n";
        let metrics = scanners::scan_go("a.go", source);
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[1].function, "branchy");
        assert_eq!(metrics[1].complexity, 2);
    }

    #[test]
    fn ignores_identifiers_that_contain_keywords() {
        assert_eq!(lang::count_keyword("iffy()", "if"), 0);
        assert_eq!(lang::count_keyword("for_each()", "for"), 0);
        assert_eq!(lang::count_keyword("if (x) {", "if"), 1);
    }
}
