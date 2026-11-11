// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The workflow flag surface.
//!
//! Two shapes live here: the Go `flag` set that `apps` and `list` use, and
//! the hand-rolled scanner the remaining subcommands use. The hand-rolled one
//! is not a simplification: Go's original parses `--timeout abc` as `0` and
//! leaves an unmatched token in the positional list, and reproducing that is
//! what keeps the two binaries interchangeable in scripts.

use crate::cli::{report_parse_error, FlagSet};

use super::model::DEFAULT_WORKFLOW_CONFIG_PATH;

/// The flags every subcommand accepts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkflowFlags {
    /// `--config PATH`: the highest-precedence topology file.
    pub config_path: String,
    /// `--dry-run`: say what would happen, change nothing.
    pub dry_run: bool,
    /// `--output text|json`.
    pub output: String,
    /// `--verbose`: print each external command before running it.
    pub verbose: bool,
}

impl WorkflowFlags {
    /// Consumes one of the shared flags, when `arg` spells one.
    pub fn take_common(&mut self, arg: &str, next: Option<&str>) -> Option<Take> {
        match (arg, next) {
            ("--config", Some(value)) => {
                self.config_path = value.to_string();
                Some(Take::Value)
            }
            ("--dry-run", _) => {
                self.dry_run = true;
                Some(Take::Rest)
            }
            ("--verbose", _) => {
                self.verbose = true;
                Some(Take::Rest)
            }
            ("--output", Some(value)) => {
                self.output = value.to_string();
                Some(Take::Value)
            }
            _ => None,
        }
    }
}

/// How a recognised flag consumed its token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Take {
    /// A switch: the next token is another flag or a positional.
    Rest,
    /// A flag with a value: the next token belongs to it.
    Value,
}

/// Scans `args` with the hand-rolled rule set: a flag that takes a value is
/// only recognised when a value follows, and every other token is positional.
pub fn parse_with<F>(args: &[String], mut on_flag: F) -> Vec<String>
where
    F: FnMut(&str, Option<&str>) -> Option<Take>,
{
    let mut rest = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        let next = args.get(index + 1).map(String::as_str);
        match on_flag(arg, next) {
            Some(Take::Value) => index += 2,
            Some(Take::Rest) => index += 1,
            None => {
                rest.push(arg.to_string());
                index += 1;
            }
        }
    }
    rest
}

/// Go's `fmt.Sscanf(value, "%d", &n)`: leading blanks, an optional sign and
/// the decimal digits that follow; anything else leaves the field at `0`.
pub fn scan_int(value: &str) -> i64 {
    let trimmed = value.trim_start();
    let (sign, rest) = match trimmed.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return 0;
    }
    digits.parse::<i64>().map(|n| sign * n).unwrap_or(0)
}

/// Parses the shared flags with Go's `flag` package, for `apps` and `list`.
///
/// A refusal prints Go's error line and the usage block on stderr; `-h`
/// prints the block alone, and both travel up as errors so `main` adds its
/// own `Error:` line (Go exits 1 on `-h` here too).
pub fn parse_common(args: &[String]) -> Result<(WorkflowFlags, Vec<String>), String> {
    let mut fs = FlagSet::new();
    fs.str_flag("config", "");
    fs.bool_flag("dry-run", false);
    fs.str_flag("output", "text");
    fs.bool_flag("verbose", false);
    fs.describe(
        "config",
        &format!("workflow config YAML (default: {DEFAULT_WORKFLOW_CONFIG_PATH})"),
    );
    fs.describe("dry-run", "show what would happen, do not push/trigger");
    fs.describe("output", "output format: text (default) | json");
    fs.describe("verbose", "verbose runner output");
    if let Err(error) = fs.parse(args) {
        report_parse_error(&error, &fs.render_usage("workflow"));
        return Err(error.to_string());
    }
    let flags = WorkflowFlags {
        config_path: fs.get("config").map(|v| v.as_str("")).unwrap_or_default(),
        dry_run: fs.get("dry-run").map(|v| v.as_bool(false)).unwrap_or(false),
        output: fs
            .get("output")
            .map(|v| v.as_str("text"))
            .unwrap_or_else(|| "text".to_string()),
        verbose: fs.get("verbose").map(|v| v.as_bool(false)).unwrap_or(false),
    };
    Ok((flags, fs.args.clone()))
}

#[cfg(test)]
mod tests;
