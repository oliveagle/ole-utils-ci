// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The MoonBit toolchain stages: compiler, formatter and tests.

use std::path::Path;
use std::time::Duration;

use crate::fsutil;
use crate::output::{Diagnostic, Report, Severity};

use super::coverage::Coverage;
use super::engine;
use super::flags::MoonbitFlags;
use super::report::Totals;

pub(super) fn run_compiler(
    flags: &MoonbitFlags,
    module: &Path,
    timeout: Duration,
    report: &mut Report,
    totals: &mut Totals,
) {
    let run = match engine::check(module, timeout, flags.deny_warnings) {
        Ok(run) => run,
        Err(error) => {
            report.add(Diagnostic::error(&flags.src, "moonbit/check", error));
            totals.syntax_errors += 1;
            return;
        }
    };
    let diagnostics = engine::parse_check(&run.stdout);
    for item in diagnostics {
        let severity = if flags.deny_warnings {
            Severity::Error
        } else {
            item.severity
        };
        let path = if item.file.is_empty() || Path::new(&item.file) == module {
            ".".to_string()
        } else {
            let path = Path::new(&item.file);
            let relative = fsutil::relative_slashed(module, path);
            if relative.starts_with('/') {
                ".".to_string()
            } else {
                relative
            }
        };
        let mut diagnostic =
            Diagnostic::error(path, "moonbit/check", item.message).at_line(item.line);
        diagnostic.severity = severity;
        diagnostic.col = item.col;
        report.add(diagnostic);
    }
    if !run.success {
        totals.syntax_errors += 1;
        if engine::parse_check(&run.stdout).is_empty() {
            report.add(Diagnostic::error(
                &flags.src,
                "moonbit/check",
                engine::first_line(&run.output),
            ));
        }
    }
}

pub(super) fn run_format(
    flags: &MoonbitFlags,
    module: &Path,
    src: &Path,
    timeout: Duration,
    report: &mut Report,
    totals: &mut Totals,
) {
    match engine::format_check(module, timeout) {
        Ok(run) if run.success => {}
        Ok(_) => {
            totals.format_errors += 1;
            let rel = fsutil::relative_slashed(module, src);
            let rel = if rel.is_empty() { ".".to_string() } else { rel };
            report.add(
                Diagnostic::error(
                    rel,
                    "moonbit/format",
                    "moon fmt --check found unformatted files; run `moon fmt` to fix".to_string(),
                )
                .at_line(1),
            );
        }
        Err(error) => {
            totals.format_errors += 1;
            report.add(Diagnostic::error(&flags.src, "moonbit/format", error));
        }
    }
}

pub(super) fn run_tests(
    flags: &MoonbitFlags,
    module: &Path,
    timeout: Duration,
    report: &mut Report,
    totals: &mut Totals,
) {
    let run = match engine::test(module, timeout, flags.coverage, &flags.test_filter) {
        Ok(run) => run,
        Err(error) => {
            report.add(Diagnostic::error(&flags.src, "moonbit/test", error));
            totals.test_faults += 1;
            return;
        }
    };
    let parsed = engine::parse_test_totals(&run.stdout);
    totals.tests_run = parsed.total;
    totals.test_faults += parsed.failed;
    for failure in engine::parse_test_failures(&run.stdout) {
        report.add(
            Diagnostic::error(
                failure.file,
                "moonbit/test",
                format!("test {} failed: {}", failure.name, failure.message),
            )
            .at_line(1),
        );
    }
    if !run.success && parsed.failed == 0 {
        totals.test_faults += 1;
        report.add(Diagnostic::error(
            &flags.src,
            "moonbit/test",
            engine::first_line(&run.output),
        ));
    }
}

pub(super) fn enforce_line_threshold(
    flags: &MoonbitFlags,
    report: &mut Report,
    coverage: &Coverage,
) {
    if coverage.percent() < flags.line_threshold {
        report.add(Diagnostic::error(
            &flags.src,
            "moonbit/line-coverage",
            format!(
                "line coverage {:.1}% is below {:.1}%",
                coverage.percent(),
                flags.line_threshold
            ),
        ));
    }
}
