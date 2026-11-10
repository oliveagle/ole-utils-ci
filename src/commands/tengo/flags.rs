// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The private flag set of the `tengo-*` commands.
//!
//! Go gives each of them its own `flag.FlagSet` with `ContinueOnError` and
//! discards the built-in usage output, so the only thing a caller ever sees
//! is the error `flag` returns. This parser reproduces that behaviour —
//! including "parsing stops at the first positional argument" — for this
//! command's own flag names, which the shared global parser must not see.

use crate::goparse;

/// Every flag `tengo-*` understands. None of them is a boolean, so each one
/// always consumes a value.
const FLAGS: &[&str] = &[
    "src",
    "cases",
    "tool",
    "output",
    "timeout",
    "max-allocs",
    "threshold",
    "line-threshold",
    "branch-threshold",
    "report",
    "junit",
    "lcov",
];

/// One `tengo-*` command's parsed flags, with Go's defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct TengoFlags {
    /// Tengo source root.
    pub src: String,
    /// Comma-separated case files or directories.
    pub cases: String,
    /// Force one tool id.
    pub tool: String,
    /// `toon`, `json` or `text`.
    pub output: String,
    /// Per-case execution timeout in nanoseconds, signed the way Go stores
    /// `time.Duration`.
    pub timeout_ns: i64,
    /// Tengo VM allocation budget.
    pub max_allocs: i64,
    /// Mutation score threshold.
    pub threshold: f64,
    /// Line coverage threshold.
    pub line_threshold: f64,
    /// Branch coverage threshold.
    pub branch_threshold: f64,
    /// Canonical JSON report path.
    pub report: String,
    /// JUnit XML report path.
    pub junit: String,
    /// LCOV report path.
    pub lcov: String,
}

impl Default for TengoFlags {
    fn default() -> Self {
        TengoFlags {
            src: "src".to_string(),
            cases: String::new(),
            tool: String::new(),
            output: "toon".to_string(),
            timeout_ns: 5_000_000_000,
            max_allocs: 1_000_000,
            threshold: 95.0,
            line_threshold: 95.0,
            branch_threshold: 95.0,
            report: String::new(),
            junit: String::new(),
            lcov: String::new(),
        }
    }
}

/// Parses `argv` the way Go's `flag` package would.
pub fn parse(argv: &[String]) -> Result<TengoFlags, String> {
    let mut values = TengoFlags::default();
    let mut index = 0;
    while index < argv.len() {
        let arg = &argv[index];
        if arg.len() < 2 || !arg.starts_with('-') {
            break;
        }
        let (body, double) = match arg.strip_prefix("--") {
            Some(rest) => (rest, true),
            None => (&arg[1..], false),
        };
        if double && body.is_empty() {
            break;
        }
        if body.is_empty() || body.starts_with('-') || body.starts_with('=') {
            return Err(format!("bad flag syntax: {arg}"));
        }
        let (flag_name, inline) = match body.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (body, None),
        };
        if !FLAGS.contains(&flag_name) {
            if flag_name == "help" || flag_name == "h" {
                return Err("flag: help requested".to_string());
            }
            return Err(format!("flag provided but not defined: -{flag_name}"));
        }
        let raw = match inline {
            Some(value) => value.to_string(),
            None => {
                index += 1;
                match argv.get(index) {
                    Some(value) => value.clone(),
                    None => return Err(format!("flag needs an argument: -{flag_name}")),
                }
            }
        };
        apply(&mut values, flag_name, &raw)?;
        index += 1;
    }
    Ok(values)
}

/// Stores one parsed value, or reports Go's wording for rejecting it.
fn apply(values: &mut TengoFlags, name: &str, raw: &str) -> Result<(), String> {
    let reason = match name {
        "src" => {
            values.src = raw.to_string();
            return Ok(());
        }
        "cases" => {
            values.cases = raw.to_string();
            return Ok(());
        }
        "tool" => {
            values.tool = raw.to_string();
            return Ok(());
        }
        "output" => {
            values.output = raw.to_string();
            return Ok(());
        }
        "report" => {
            values.report = raw.to_string();
            return Ok(());
        }
        "junit" => {
            values.junit = raw.to_string();
            return Ok(());
        }
        "lcov" => {
            values.lcov = raw.to_string();
            return Ok(());
        }
        "timeout" => match goparse::parse_duration(raw) {
            Some(nanos) => {
                values.timeout_ns = nanos;
                return Ok(());
            }
            None => goparse::NumError::Syntax.as_str(),
        },
        "max-allocs" => match goparse::parse_int(raw) {
            Ok(value) => {
                values.max_allocs = value;
                return Ok(());
            }
            Err(error) => error.as_str(),
        },
        "threshold" => match goparse::parse_float(raw) {
            Ok(value) => {
                values.threshold = value;
                return Ok(());
            }
            Err(error) => error.as_str(),
        },
        "line-threshold" => match goparse::parse_float(raw) {
            Ok(value) => {
                values.line_threshold = value;
                return Ok(());
            }
            Err(error) => error.as_str(),
        },
        "branch-threshold" => match goparse::parse_float(raw) {
            Ok(value) => {
                values.branch_threshold = value;
                return Ok(());
            }
            Err(error) => error.as_str(),
        },
        // `FLAGS` only registers the names handled above.
        other => {
            return Err(format!(
                "invalid value {raw:?} for flag -{other}: parse error"
            ))
        }
    };
    Err(format!("invalid value {raw:?} for flag -{name}: {reason}"))
}

/// Go's `splitCSV`: trim each item and drop the empties.
pub fn split_csv(value: &str) -> Vec<String> {
    if value.trim().is_empty() {
        return Vec::new();
    }
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn stops_at_the_first_positional() {
        let values = parse(&strings(&["--src", "a", "b", "--lcov", "x"])).unwrap();
        assert_eq!(values.src, "a");
        assert!(values.lcov.is_empty());
    }

    #[test]
    fn a_missing_inline_value_consumes_the_next_argument() {
        let values = parse(&strings(&["--src", "--lcov"])).unwrap();
        assert_eq!(values.src, "--lcov");
    }

    #[test]
    fn reports_go_wording_for_bad_input() {
        let cases = [
            (
                strings(&["--bogus"]),
                "flag provided but not defined: -bogus",
            ),
            (strings(&["-h"]), "flag: help requested"),
            (strings(&["--help"]), "flag: help requested"),
            (strings(&["-src"]), "flag needs an argument: -src"),
            (
                strings(&["--timeout", "abc"]),
                "invalid value \"abc\" for flag -timeout: parse error",
            ),
            (
                strings(&["--max-allocs", "99999999999999999999"]),
                "invalid value \"99999999999999999999\" for flag -max-allocs: value out of range",
            ),
            (strings(&["---foo"]), "bad flag syntax: ---foo"),
        ];
        for (args, expected) in cases {
            assert_eq!(parse(&args).unwrap_err(), expected);
        }
    }

    #[test]
    fn splits_case_lists_the_way_go_does() {
        assert_eq!(split_csv(""), Vec::<String>::new());
        assert_eq!(split_csv(" , "), Vec::<String>::new());
        assert_eq!(split_csv(" a , ,b "), strings(&["a", "b"]));
    }
}
