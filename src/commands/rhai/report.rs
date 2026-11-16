// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The Rhai gate's per-file payload, totals and case-selection guard.

use std::path::Path;

use crate::fsutil;
use crate::output::{Diagnostic, Report};

use super::cases::Case;
use super::coverage::CoverageStatus;
use super::engine::{self, LoadedSource, SourceError};

pub(super) struct FileResult {
    pub(super) value: serde_json::Value,
    pub(super) syntax_error: Option<SourceError>,
}

impl FileResult {
    pub(super) fn new(rel: &str) -> Self {
        Self {
            value: serde_json::json!({ "file": rel }),
            syntax_error: None,
        }
    }

    pub(super) fn with_syntax_error(mut self, error: &SourceError) -> Self {
        self.syntax_error = Some((*error).clone());
        self.value["syntax"] = serde_json::json!({
            "col": error.col,
            "error": error.message,
            "line": error.line,
            "ok": false,
        });
        self
    }
}

/// Coverage totals across all files.
#[derive(Debug, Default)]
pub(super) struct CoverageTotals {
    pub(super) syntax_errors: usize,
    pub(super) test_faults: usize,
    pub(super) complexity_errors: usize,
    pub(super) coverage_instrumentation_errors: usize,
    pub(super) line_covered: usize,
    pub(super) line_total: usize,
    pub(super) branch_covered: usize,
    pub(super) branch_total: usize,
    pub(super) line_percent: f64,
    pub(super) branch_percent: f64,
}

/// Appends a complete per-file object, preserving diagnostic order.
pub(super) fn append_metrics(
    files: &mut Vec<serde_json::Value>,
    file: FileResult,
    loaded: Option<LoadedSource>,
    metrics: Option<Vec<crate::gates::complexity::lang::FunctionMetric>>,
    result: Option<ResultPayload>,
) {
    let mut value = file.value;
    if file.syntax_error.is_none() {
        value["syntax"] = serde_json::json!({ "ok": true });
    }
    if let Some(loaded) = loaded {
        if let Some(message) = loaded.coverage_error {
            value["coverageError"] = serde_json::Value::String(message);
        }
        value["branches"] = serde_json::json!(loaded
            .branches
            .iter()
            .map(|site| serde_json::json!({
                "col": site.col,
                "id": site.id,
                "line": site.line,
            }))
            .collect::<Vec<_>>());
        value["executableLines"] =
            serde_json::json!(loaded.lines.iter().copied().collect::<Vec<usize>>());
    }
    if let Some(metrics) = metrics {
        value["complexity"] = serde_json::json!(metrics
            .iter()
            .map(|metric| serde_json::json!({
                "complexity": metric.complexity,
                "function": metric.function,
                "line": metric.line,
            }))
            .collect::<Vec<_>>());
    }
    if let Some(result) = result {
        value["cases"] = serde_json::json!(result.cases);
        value["lineCoverage"] = serde_json::json!({
            "covered": result.line.covered_sites,
            "missingLines": result.line.missing_sites,
            "percent": result.line.percent,
            "threshold": result.line.threshold,
            "total": result.line.total_sites,
        });
        value["branchCoverage"] = serde_json::json!({
            "coveredOutcomes": result.branch.covered_sites,
            "missingOutcomes": result.branch.missing_outcomes,
            "missingSites": result.branch.missing_sites,
            "percent": result.branch.percent,
            "threshold": result.branch.threshold,
            "totalOutcomes": result.branch.total_sites,
        });
    }
    files.push(value);
}

/// The per-file payload attached to the report.
pub(super) struct ResultPayload {
    pub(super) cases: Vec<serde_json::Value>,
    pub(super) line: CoverageStatus,
    pub(super) branch: CoverageStatus,
}

/// A safe percentage with the usual empty-set convention.
pub(super) fn percent(covered: usize, total: usize) -> f64 {
    if total == 0 {
        100.0
    } else {
        f64::from(covered as u32) * 100.0 / f64::from(total as u32)
    }
}

impl SourceError {
    /// Converts the engine error to a gate diagnostic.
    pub(super) fn to_diagnostic(&self, path: &str) -> Diagnostic {
        let mut diagnostic =
            Diagnostic::error(path, self.rule.clone(), self.message.clone()).at_line(self.line);
        diagnostic.col = self.col;
        diagnostic
    }
}

/// Guards the case selection: a case must name a selected file.
pub(super) fn check_case_selection(
    root: &Path,
    sources: &[fsutil::Entry],
    test_cases: &[Case],
    report: &mut Report,
    totals: &mut CoverageTotals,
) {
    let named_cases = test_cases
        .iter()
        .filter(|case| case.selected_file().is_some())
        .count();
    if named_cases > 0 {
        let known = sources
            .iter()
            .map(|entry| entry.rel.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for case in test_cases {
            if let Some(name) = case.selected_file() {
                let selected = engine::resolve_source(root, name)
                    .map(|path| fsutil::relative_slashed(root, &path));
                if selected.as_deref() != Ok(name) || !known.contains(name) {
                    report.add(Diagnostic::error(
                        name,
                        "rhai/case-file",
                        "coverage case names a file not selected by --src",
                    ));
                    totals.test_faults += 1;
                }
            }
        }
    } else if !sources.is_empty() {
        for source in sources {
            report.add(Diagnostic::error(
                &source.rel,
                "rhai/no-cases",
                "no coverage case calls this file",
            ));
            totals.test_faults += 1;
        }
    }
}
