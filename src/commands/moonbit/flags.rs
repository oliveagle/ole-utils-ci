// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The private flag set of `moonbit-check`.

use crate::cli::{FlagSet, FlagValue};

/// The default number of files one run inspects.
pub const DEFAULT_MAX_FILES: i64 = 200;
/// The default largest source file, in bytes.
pub const DEFAULT_MAX_FILE_SIZE: i64 = 1_048_576;

/// One parsed `moonbit-check` command line.
#[derive(Debug, Clone, PartialEq)]
pub struct MoonbitFlags {
    /// Root walked for `.mbt` files; a `moon.mod` at or above it is required.
    pub src: String,
    /// `text`, `json`, or `toon`.
    pub output: String,
    /// Wall-clock budget for one `moon` invocation.
    pub timeout_ns: i64,
    /// Line coverage percentage floor.
    pub line_threshold: f64,
    /// Complexity warning budget.
    pub complexity_warn: usize,
    /// Complexity hard cap.
    pub max_complexity: usize,
    /// Maximum files checked in one run.
    pub max_files: i64,
    /// Maximum source-file size in bytes.
    pub max_file_size: i64,
    /// Run `moon fmt --check`.
    pub check_format: bool,
    /// Run `moon test`.
    pub run_tests: bool,
    /// Instrument the tests and turn the Cobertura report into line coverage.
    pub coverage: bool,
    /// Pass `-d` / `--deny-warn` to `moon check`.
    pub deny_warnings: bool,
    /// Optional `moon test --filter` glob.
    pub test_filter: String,
}

impl Default for MoonbitFlags {
    fn default() -> Self {
        Self {
            src: ".".to_string(),
            output: "toon".to_string(),
            timeout_ns: 300_000_000_000,
            line_threshold: 80.0,
            complexity_warn: 15,
            max_complexity: 20,
            max_files: DEFAULT_MAX_FILES,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            check_format: true,
            run_tests: true,
            coverage: true,
            deny_warnings: true,
            test_filter: String::new(),
        }
    }
}

/// Builds the Go-style private flag set.
pub fn flag_set() -> FlagSet {
    let defaults = MoonbitFlags::default();
    let mut fs = FlagSet::new();
    fs.str_flag("src", &defaults.src)
        .str_flag("output", &defaults.output)
        .duration_flag("timeout", "5m")
        .float_flag("line-threshold", defaults.line_threshold)
        .int_flag("complexity-warn", defaults.complexity_warn as i64)
        .int_flag("max-complexity", defaults.max_complexity as i64)
        .int_flag("max-files", defaults.max_files)
        .int_flag("max-file-size", defaults.max_file_size)
        .bool_flag("check-format", defaults.check_format)
        .bool_flag("run-tests", defaults.run_tests)
        .bool_flag("coverage", defaults.coverage)
        .bool_flag("deny-warnings", defaults.deny_warnings)
        .str_flag("test-filter", &defaults.test_filter)
        .describe("src", "source root searched for .mbt files")
        .describe("output", "text|json|toon")
        .describe("timeout", "per-command wall-clock budget")
        .describe("line-threshold", "line coverage floor (percent)")
        .describe("complexity-warn", "complexity budget")
        .describe("max-complexity", "complexity hard cap")
        .describe("max-files", "maximum files checked")
        .describe("max-file-size", "maximum source size in bytes")
        .describe("check-format", "run moon fmt --check")
        .describe("run-tests", "run moon test")
        .describe(
            "coverage",
            "instrument tests and read Cobertura line coverage",
        )
        .describe("deny-warnings", "treat moon check warnings as errors")
        .describe("test-filter", "moon test --filter glob");
    fs
}

/// Reads the parsed values back out.
pub fn from_flag_set(fs: &FlagSet) -> MoonbitFlags {
    let defaults = MoonbitFlags::default();
    let text = |name: &str| fs.get(name).map(|v| v.as_str("")).unwrap_or_default();
    let int =
        |name: &str, fallback: i64| fs.get(name).map(|v| v.as_int(fallback)).unwrap_or(fallback);
    let boolean = |name: &str, fallback: bool| {
        fs.get(name)
            .map(|v: &FlagValue| v.as_bool(fallback))
            .unwrap_or(fallback)
    };
    MoonbitFlags {
        src: text("src"),
        output: text("output"),
        timeout_ns: fs
            .get("timeout")
            .map(|v| v.as_duration(defaults.timeout_ns))
            .unwrap_or(defaults.timeout_ns),
        line_threshold: fs
            .get("line-threshold")
            .map(|v| v.as_float(defaults.line_threshold))
            .unwrap_or(defaults.line_threshold),
        complexity_warn: int("complexity-warn", defaults.complexity_warn as i64).max(0) as usize,
        max_complexity: int("max-complexity", defaults.max_complexity as i64).max(0) as usize,
        max_files: int("max-files", defaults.max_files),
        max_file_size: int("max-file-size", defaults.max_file_size),
        check_format: boolean("check-format", defaults.check_format),
        run_tests: boolean("run-tests", defaults.run_tests),
        coverage: boolean("coverage", defaults.coverage),
        deny_warnings: boolean("deny-warnings", defaults.deny_warnings),
        test_filter: text("test-filter"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn parses_private_flags() {
        let mut fs = flag_set();
        fs.parse(&args(&[
            "--src",
            "module",
            "--line-threshold=90",
            "--coverage=false",
            "--test-filter",
            "kernel*",
        ]))
        .expect("valid flags");
        let values = from_flag_set(&fs);
        assert_eq!(values.src, "module");
        assert_eq!(values.line_threshold, 90.0);
        assert!(!values.coverage);
        assert_eq!(values.test_filter, "kernel*");
    }

    #[test]
    fn unknown_flags_fail_before_the_gate() {
        let mut fs = flag_set();
        let error = fs.parse(&args(&["--nope"])).unwrap_err();
        assert_eq!(error.to_string(), "flag provided but not defined: -nope");
    }
}
