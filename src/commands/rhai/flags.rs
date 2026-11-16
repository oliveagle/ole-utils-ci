// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The private flag set for `rhai-check`.
//!
//! The command follows the focused lint commands: its flags never reach the
//! shared parser, and machine output defaults to TOON so an agent can read it.

use crate::cli::FlagSet;

/// The default per-run file cap.
pub const DEFAULT_MAX_FILES: i64 = 200;

/// The default per-file byte cap.
pub const DEFAULT_MAX_FILE_SIZE: i64 = 1_048_576;

/// One parsed `rhai-check` command line.
#[derive(Debug, Clone, PartialEq)]
pub struct RhaiFlags {
    /// Root walked for `.rhai` files.
    pub src: String,
    /// JSON test-case file or directory.
    pub cases: String,
    /// `text`, `json`, or `toon`.
    pub output: String,
    /// Advised execution budget, recorded with a slow-case warning.
    pub timeout_ns: i64,
    /// Rhai operation budget per test call.
    pub max_operations: i64,
    /// Line coverage percentage floor.
    pub line_threshold: f64,
    /// Branch coverage percentage floor.
    pub branch_threshold: f64,
    /// Complexity warning budget.
    pub complexity_warn: usize,
    /// Complexity hard cap.
    pub max_complexity: usize,
    /// Maximum files checked in one run.
    pub max_files: i64,
    /// Maximum source-file size in bytes.
    pub max_file_size: i64,
}

impl Default for RhaiFlags {
    fn default() -> Self {
        Self {
            src: ".".to_string(),
            cases: String::new(),
            output: "toon".to_string(),
            timeout_ns: 5_000_000_000,
            max_operations: 2_000_000,
            line_threshold: 80.0,
            branch_threshold: 80.0,
            complexity_warn: 15,
            max_complexity: 20,
            max_files: DEFAULT_MAX_FILES,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
        }
    }
}

/// Builds the Go-style private flag set.
pub fn flag_set() -> FlagSet {
    let defaults = RhaiFlags::default();
    let mut fs = FlagSet::new();
    fs.str_flag("src", &defaults.src)
        .str_flag("cases", &defaults.cases)
        .str_flag("output", &defaults.output)
        .duration_flag("timeout", "5s")
        .int_flag("max-operations", defaults.max_operations)
        .float_flag("line-threshold", defaults.line_threshold)
        .float_flag("branch-threshold", defaults.branch_threshold)
        .int_flag("complexity-warn", defaults.complexity_warn as i64)
        .int_flag("max-complexity", defaults.max_complexity as i64)
        .int_flag("max-files", defaults.max_files)
        .int_flag("max-file-size", defaults.max_file_size)
        .describe("src", "source root searched for .rhai files")
        .describe("cases", "JSON test-case file or directory")
        .describe("output", "text|json|toon")
        .describe("timeout", "per-call advisory duration budget")
        .describe("max-operations", "Rhai operation budget per call")
        .describe("line-threshold", "line coverage floor (percent)")
        .describe("branch-threshold", "branch coverage floor (percent)")
        .describe("complexity-warn", "complexity budget")
        .describe("max-complexity", "complexity hard cap")
        .describe("max-files", "maximum files checked")
        .describe("max-file-size", "maximum source size in bytes");
    fs
}

/// Reads the parsed values back out.
pub fn from_flag_set(fs: &FlagSet) -> RhaiFlags {
    let defaults = RhaiFlags::default();
    let text = |name: &str| {
        fs.get(name)
            .map(|value| value.as_str(""))
            .unwrap_or_default()
    };
    let int = |name: &str, fallback: i64| {
        fs.get(name)
            .map(|value| value.as_int(fallback))
            .unwrap_or(fallback)
    };
    let float = |name: &str, fallback: f64| {
        fs.get(name)
            .map(|value| value.as_float(fallback))
            .unwrap_or(fallback)
    };
    RhaiFlags {
        src: text("src"),
        cases: text("cases"),
        output: text("output"),
        timeout_ns: int("timeout", defaults.timeout_ns),
        max_operations: int("max-operations", defaults.max_operations),
        line_threshold: float("line-threshold", defaults.line_threshold),
        branch_threshold: float("branch-threshold", defaults.branch_threshold),
        complexity_warn: int("complexity-warn", defaults.complexity_warn as i64).max(0) as usize,
        max_complexity: int("max-complexity", defaults.max_complexity as i64).max(0) as usize,
        max_files: int("max-files", defaults.max_files),
        max_file_size: int("max-file-size", defaults.max_file_size),
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
            "examples",
            "--line-threshold=90",
            "--complexity-warn",
            "10",
        ]))
        .expect("valid flags");
        let values = from_flag_set(&fs);
        assert_eq!(values.src, "examples");
        assert_eq!(values.line_threshold, 90.0);
        assert_eq!(values.complexity_warn, 10);
    }

    #[test]
    fn unknown_flags_fail_before_the_gate() {
        let mut fs = flag_set();
        let error = fs.parse(&args(&["--nope"])).unwrap_err();
        assert_eq!(error.to_string(), "flag provided but not defined: -nope");
    }
}
