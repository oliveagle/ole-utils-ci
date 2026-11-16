// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `moonbit-check` gate and its structured payload.

use std::path::Path;
use std::time::Duration;

use crate::cli::{report_parse_error, ParseError};
use crate::fsutil;
use crate::output::{Diagnostic, Report};

use super::coverage::{self, Coverage};
use super::engine;
use super::flags::{flag_set, from_flag_set, MoonbitFlags};
use super::report::{self, Totals};
use super::usage::FRIENDLY_USAGE;
use super::{sources, stages};

/// The command entry point. `args` follows `moonbit-check`.
pub fn moonbit_check(args: &[String]) -> Result<i32, String> {
    let mut fs = flag_set();
    if let Err(error) = fs.parse(args) {
        report_parse_error(&error, &fs.render_usage("moonbit-check"));
        if matches!(error, ParseError::Help) {
            println!("{FRIENDLY_USAGE}");
            return Ok(0);
        }
        eprintln!("Error: {error}");
        return Ok(2);
    }
    let flags = from_flag_set(&fs);
    if let Err(message) = crate::output::validate_mode(&flags.output) {
        eprintln!("moonbit-check: {message}");
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

/// Runs the focused MoonBit gate.
pub fn run(flags: &MoonbitFlags) -> Result<Report, String> {
    validate(flags)?;
    let src = fsutil::absolute(Path::new(&flags.src))
        .map_err(|error| format!("resolve source root: {error}"))?;
    if !src.is_dir() {
        return Err(format!("source root is not a directory: {}", src.display()));
    }
    let mut report = Report::new("moonbit-check");
    if !engine::tool_available() {
        report.add(Diagnostic::error(
            &flags.src,
            "moonbit/toolchain",
            "the moon executable is not available on PATH",
        ));
        return report::finish(
            flags,
            report,
            Vec::new(),
            Totals::default(),
            Coverage::default(),
            false,
        );
    }
    let Some(module) = engine::module_root(&src) else {
        report.add(Diagnostic::error(
            &flags.src,
            "moonbit/no-module",
            "no moon.mod found at or above the source root",
        ));
        return report::finish(
            flags,
            report,
            Vec::new(),
            Totals::default(),
            Coverage::default(),
            false,
        );
    };
    let sources = engine::discover_sources(&src);
    let timeout = Duration::from_nanos(flags.timeout_ns.max(1) as u64);
    let mut totals = Totals::default();
    let mut files = Vec::new();
    if sources.is_empty() {
        report.add(Diagnostic::error(
            &flags.src,
            "moonbit/no-files",
            "no .mbt files found",
        ));
    }
    for source in sources.iter().take(flags.max_files.max(1) as usize) {
        sources::check_source(flags, source, &mut report, &mut totals, &mut files);
    }
    stages::run_compiler(flags, &module, timeout, &mut report, &mut totals);
    if flags.check_format {
        stages::run_format(flags, &module, &src, timeout, &mut report, &mut totals);
    }
    let syntax_ok = totals.syntax_errors == 0;
    if flags.run_tests && syntax_ok {
        stages::run_tests(flags, &module, timeout, &mut report, &mut totals);
    }
    let mut coverage = Coverage::default();
    let mut coverage_ran = false;
    if flags.coverage && flags.run_tests && syntax_ok && totals.tests_run > 0 {
        match engine::coverage_report(&module, timeout) {
            Ok(xml) => {
                coverage = coverage::parse(&xml);
                coverage_ran = true;
                stages::enforce_line_threshold(flags, &mut report, &coverage);
            }
            Err(error) => {
                report.add(Diagnostic::error(&flags.src, "moonbit/coverage", error));
                totals.coverage_errors += 1;
            }
        }
    }
    report::finish(flags, report, files, totals, coverage, coverage_ran)
}

fn validate(flags: &MoonbitFlags) -> Result<(), String> {
    if flags.max_files < 1 {
        return Err("--max-files must be at least 1".to_string());
    }
    if flags.max_file_size < 1 {
        return Err("--max-file-size must be at least 1".to_string());
    }
    if !(0.0..=100.0).contains(&flags.line_threshold) {
        return Err("--line-threshold must be between 0 and 100".to_string());
    }
    if flags.complexity_warn == 0
        || flags.max_complexity == 0
        || flags.complexity_warn > flags.max_complexity
    {
        return Err("complexity budgets must satisfy 0 < warn <= max".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_line_thresholds() {
        let flags = MoonbitFlags {
            line_threshold: 101.0,
            ..MoonbitFlags::default()
        };
        assert_eq!(
            validate(&flags).unwrap_err(),
            "--line-threshold must be between 0 and 100"
        );
    }
}
