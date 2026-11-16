// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The MoonBit gate's structured payload.

use crate::output::Report;

use super::coverage::Coverage;
use super::flags::MoonbitFlags;

pub(super) fn finish(
    flags: &MoonbitFlags,
    mut report: Report,
    files: Vec<serde_json::Value>,
    totals: Totals,
    coverage: Coverage,
    coverage_ran: bool,
) -> Result<Report, String> {
    let coverage_enabled = flags.coverage && flags.run_tests;
    let line_percent = coverage_ran.then(|| coverage.percent());
    let line_label = line_percent
        .map(|percent| format!("line {percent:.1}%"))
        .unwrap_or_else(|| "line n/a".to_string());
    report.summary = format!(
        "moonbit-check: {} file(s), {} syntax error(s), {} format error(s), {} test fault(s), {} test(s), {}, {} complexity error(s)",
        files.len(),
        totals.syntax_errors,
        totals.format_errors,
        totals.test_faults,
        totals.tests_run,
        line_label,
        totals.complexity_errors,
    );
    report.extra = Some(serde_json::json!({
        "files": files,
        "summary": {
            "complexityErrors": totals.complexity_errors,
            "coverageErrors": totals.coverage_errors,
            "files": files.len(),
            "formatErrors": totals.format_errors,
            "lineCovered": coverage.covered,
            "lineCoveragePercent": line_percent,
            "lineTotal": coverage.total,
            "syntaxErrors": totals.syntax_errors,
            "testFaults": totals.test_faults,
            "testsRun": totals.tests_run,
        },
        "thresholds": {
            "complexityWarn": flags.complexity_warn,
            "lineCoverage": flags.line_threshold,
            "maxComplexity": flags.max_complexity,
        },
        "coverage": {
            "branchCoverageSupported": false,
            "enabled": coverage_enabled,
            "reported": coverage_ran,
            "files": coverage.files,
        },
    }));
    Ok(report)
}

#[derive(Debug, Default)]
pub(super) struct Totals {
    pub(super) syntax_errors: usize,
    pub(super) format_errors: usize,
    pub(super) test_faults: usize,
    pub(super) tests_run: usize,
    pub(super) complexity_errors: usize,
    pub(super) coverage_errors: usize,
}
