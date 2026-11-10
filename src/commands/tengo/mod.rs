// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `tengo-*` commands: tool unit tests, coverage and mutation testing.
//!
//! The Go original embeds the whole `pkg/tengoqa` engine, so it can parse a
//! suite and execute it in-process. This rewrite reproduces the command
//! surface — the private flag set, the output-mode contract and the case
//! discovery — but reports clearly when execution would need a Tengo VM that
//! this build does not carry.

mod cases;
mod flags;

use crate::commands::ExitCode;
use crate::output;

/// `tengo-test`: run the suite and report pass/fail per case.
pub fn tengo_test(args: &[String]) -> Result<ExitCode, String> {
    let values = flags::parse(args)?;
    output::validate_mode(&values.output)?;
    if !values.lcov.is_empty() {
        return Err("--lcov is only supported by tengo-coverage".to_string());
    }
    execute(&values)
}

/// `tengo-coverage`: enforce the line and branch coverage thresholds.
pub fn tengo_coverage(args: &[String]) -> Result<ExitCode, String> {
    let values = flags::parse(args)?;
    output::validate_mode(&values.output)?;
    if !values.junit.is_empty() {
        return Err("--junit is only supported by tengo-test".to_string());
    }
    execute(&values)
}

/// `tengo-mutate`: measure how many mutants survive the suite.
pub fn tengo_mutate(args: &[String]) -> Result<ExitCode, String> {
    let values = flags::parse(args)?;
    output::validate_mode(&values.output)?;
    if !values.junit.is_empty() {
        return Err("--junit is only supported by tengo-test".to_string());
    }
    if !values.lcov.is_empty() {
        return Err("--lcov is only supported by tengo-coverage".to_string());
    }
    execute(&values)
}

/// Resolves the suite; running it needs the Tengo VM this build lacks.
fn execute(values: &flags::TengoFlags) -> Result<ExitCode, String> {
    let requested = flags::split_csv(&values.cases);
    let files = cases::discover(&values.src, &requested)?;
    if files.is_empty() {
        return Err(format!("no Tengo test cases found under {}", values.src));
    }
    Err(
        "Tengo execution is not available: the Rust rewrite does not embed the Tengo engine"
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn rejects_report_paths_belonging_to_a_sibling_command() {
        assert_eq!(
            tengo_test(&strings(&["--lcov", "x"])).unwrap_err(),
            "--lcov is only supported by tengo-coverage"
        );
        assert_eq!(
            tengo_coverage(&strings(&["--junit", "x"])).unwrap_err(),
            "--junit is only supported by tengo-test"
        );
        assert_eq!(
            tengo_mutate(&strings(&["--junit", "x"])).unwrap_err(),
            "--junit is only supported by tengo-test"
        );
    }

    #[test]
    fn validates_the_output_mode_before_the_report_paths() {
        assert_eq!(
            tengo_test(&strings(&["--output", "bogus", "--lcov", "x"])).unwrap_err(),
            "unknown --output value \"bogus\" — use text, json, or toon"
        );
    }

    #[test]
    fn an_empty_suite_reports_the_source_root_it_walked() {
        assert_eq!(
            tengo_test(&strings(&["--src", "src"])).unwrap_err(),
            "no Tengo test cases found under src"
        );
    }
}
