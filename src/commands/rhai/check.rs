// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `rhai-check` gate and its structured payload.
//!
//! The command combines four independent answers into one exit code: Rhai
//! syntax, observed line coverage, observed branch outcomes, and cyclomatic
//! complexity. Test faults and coverage misses are errors, not warnings.

use std::path::Path;

use crate::cli::{report_parse_error, ParseError};
use crate::fsutil;
use crate::gates::complexity;
use crate::output::{Diagnostic, Report};

use super::cases;
use super::cases::Case;
use super::coverage::{branch_coverage_status, coverage_status};
use super::engine;
use super::execute::execute_cases;
use super::flags::{flag_set, from_flag_set, RhaiFlags};
use super::report::{self, append_metrics, percent, CoverageTotals, FileResult, ResultPayload};
use super::usage::FRIENDLY_USAGE;
use super::validate::validate;

/// The command entry point. `args` follows `rhai-check`.
pub fn rhai_check(args: &[String]) -> Result<i32, String> {
    let mut fs = flag_set();
    if let Err(error) = fs.parse(args) {
        report_parse_error(&error, &fs.render_usage("rhai-check"));
        if matches!(error, ParseError::Help) {
            println!("{FRIENDLY_USAGE}");
            return Ok(0);
        }
        eprintln!("Error: {error}");
        return Ok(2);
    }
    let flags = from_flag_set(&fs);
    if let Err(message) = crate::output::validate_mode(&flags.output) {
        eprintln!("rhai-check: {message}");
        eprintln!("Error: {message}");
        return Ok(2);
    }
    match run(&flags) {
        Ok(report) => {
            print!(
                "{}",
                report.render(crate::output::parse_mode(&flags.output))
            );
            Ok(i32::from(!report.ok))
        }
        Err(message) => Err(message),
    }
}

/// Runs the focused Rhai gate.
pub fn run(flags: &RhaiFlags) -> Result<Report, String> {
    let root = fsutil::absolute(Path::new(&flags.src))
        .map_err(|error| format!("resolve source root: {error}"))?;
    if !root.is_dir() {
        return Err(format!(
            "source root is not a directory: {}",
            root.display()
        ));
    }
    validate(flags)?;

    let test_cases = cases::load(&flags.cases)?;
    let sources = collect_sources(&root, flags);

    let mut report = Report::new("rhai-check");
    let mut file_results = Vec::new();
    let mut totals = CoverageTotals::default();
    if sources.is_empty() {
        report.add(Diagnostic::error(
            flags.src.clone(),
            "rhai/no-files",
            "no .rhai files found",
        ));
    }

    for source in &sources {
        check_file(
            source,
            flags,
            &test_cases,
            &mut report,
            &mut file_results,
            &mut totals,
        );
    }

    report::check_case_selection(&root, &sources, &test_cases, &mut report, &mut totals);
    Ok(finish(flags, report, file_results, totals))
}

/// Selects the `.rhai` sources under `--src`, honouring `--max-files`.
fn collect_sources(root: &Path, flags: &RhaiFlags) -> Vec<fsutil::Entry> {
    fsutil::walk_files(root)
        .into_iter()
        .filter(|entry| entry.extension() == ".rhai")
        .take(flags.max_files as usize)
        .collect()
}

/// Checks one source file and folds its result into the running totals.
fn check_file(
    source: &fsutil::Entry,
    flags: &RhaiFlags,
    test_cases: &[Case],
    report: &mut Report,
    file_results: &mut Vec<serde_json::Value>,
    totals: &mut CoverageTotals,
) {
    let file = FileResult::new(&source.rel);
    let size = std::fs::metadata(&source.path)
        .map(|meta| meta.len())
        .unwrap_or(0);
    if size > flags.max_file_size as u64 {
        report.add(
            Diagnostic::error(
                &source.rel,
                "rhai/file-size",
                format!(
                    "source is {} bytes, over --max-file-size {}",
                    size, flags.max_file_size
                ),
            )
            .at_line(1),
        );
        append_metrics(file_results, file, None, None, None);
        return;
    }
    let loaded = match engine::load(&source.path) {
        Ok(loaded) => loaded,
        Err(error) => {
            let file = file.with_syntax_error(&error);
            totals.syntax_errors += 1;
            report.add(error.to_diagnostic(&source.rel));
            append_metrics(file_results, file, None, None, None);
            return;
        }
    };

    if let Some(message) = &loaded.coverage_error {
        report.add(
            Diagnostic::error(&source.rel, "rhai/coverage-instrumentation", message)
                .at_line(loaded.branches.first().map_or(0, |branch| branch.line)),
        );
    }

    // Parse already succeeded; complexity therefore only reflects real
    // function definitions and cannot be poisoned by a lexer false hit.
    let metrics = complexity::lang::scan_file(&source.rel, &loaded.source, "rhai");
    totals.complexity_errors += add_complexity_errors(&source.rel, &metrics, flags, report);

    let file_cases: Vec<&Case> = test_cases
        .iter()
        .filter(|case| case.selected_file().is_some_and(|name| name == source.rel))
        .collect();
    let execution = execute_cases(&loaded, &file_cases, flags, &source.rel, report);
    let line = coverage_status(
        &loaded.lines.iter().copied().collect::<Vec<_>>(),
        &execution.coverage.lines.iter().copied().collect::<Vec<_>>(),
        flags.line_threshold,
    );
    let branch = branch_coverage_status(
        &loaded.branches,
        &execution.coverage.branches,
        flags.branch_threshold,
    );
    totals.syntax_errors += usize::from(file.syntax_error.is_some());
    totals.test_faults += execution.faults;
    totals.line_covered += line.covered_sites;
    totals.line_total += line.total_sites;
    totals.branch_covered += branch.covered_sites;
    totals.branch_total += branch.total_sites;
    totals.coverage_instrumentation_errors += usize::from(loaded.coverage_error.is_some());

    append_metrics(
        file_results,
        file,
        Some(loaded),
        Some(metrics),
        Some(ResultPayload {
            cases: execution.case_results,
            line,
            branch,
        }),
    );
}

/// Reports every function above the complexity budget, returning how many.
fn add_complexity_errors(
    rel: &str,
    metrics: &[complexity::lang::FunctionMetric],
    flags: &RhaiFlags,
    report: &mut Report,
) -> usize {
    let mut errors = 0usize;
    for metric in metrics
        .iter()
        .filter(|m| m.complexity > flags.complexity_warn)
    {
        let hard = metric.complexity > flags.max_complexity;
        let mut diagnostic = Diagnostic::error(
            rel,
            "rhai/complexity",
            format!(
                "{} has complexity {} over budget {}",
                metric.function, metric.complexity, flags.complexity_warn
            ),
        )
        .at_line(metric.line);
        diagnostic.detail = if hard {
            format!("also over the hard cap {}", flags.max_complexity)
        } else {
            format!("hard cap is {}", flags.max_complexity)
        };
        report.add(diagnostic);
        errors += 1;
    }
    errors
}

/// Fills the totals, coverage diagnostics, summary and JSON payload.
fn finish(
    flags: &RhaiFlags,
    mut report: Report,
    file_results: Vec<serde_json::Value>,
    mut totals: CoverageTotals,
) -> Report {
    totals.line_percent = percent(totals.line_covered, totals.line_total);
    totals.branch_percent = percent(totals.branch_covered, totals.branch_total);
    if totals.line_percent < flags.line_threshold {
        report.add(Diagnostic::error(
            &flags.src,
            "rhai/line-coverage",
            format!(
                "line coverage {:.1}% is below {:.1}%",
                totals.line_percent, flags.line_threshold
            ),
        ));
    }
    if totals.branch_percent < flags.branch_threshold {
        report.add(Diagnostic::error(
            &flags.src,
            "rhai/branch-coverage",
            format!(
                "branch coverage {:.1}% is below {:.1}%",
                totals.branch_percent, flags.branch_threshold
            ),
        ));
    }

    report.summary = format!(
        "rhai-check: {} file(s), {} syntax error(s), {} test fault(s), line {:.1}%, branch {:.1}%, {} complexity error(s)",
        file_results.len(),
        totals.syntax_errors,
        totals.test_faults,
        totals.line_percent,
        totals.branch_percent,
        totals.complexity_errors,
    );
    report.extra = Some(serde_json::json!({
        "files": file_results,
        "summary": {
            "branchCoveragePercent": totals.branch_percent,
            "branchCovered": totals.branch_covered,
            "branchTotal": totals.branch_total,
            "complexityErrors": totals.complexity_errors,
            "coverageInstrumentationErrors": totals.coverage_instrumentation_errors,
            "files": file_results.len(),
            "lineCoveragePercent": totals.line_percent,
            "lineCovered": totals.line_covered,
            "lineTotal": totals.line_total,
            "syntaxErrors": totals.syntax_errors,
            "testFaults": totals.test_faults,
        },
        "thresholds": {
            "branchCoverage": flags.branch_threshold,
            "complexityWarn": flags.complexity_warn,
            "lineCoverage": flags.line_threshold,
            "maxComplexity": flags.max_complexity,
        },
    }));
    report
}
