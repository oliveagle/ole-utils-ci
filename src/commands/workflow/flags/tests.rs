// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the two flag surfaces: the Go-`flag`-compatible one used by
//! `apps`/`list`, and the hand-rolled scanner the other subcommands use.

use super::*;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn scan_int_matches_go_sscanf_semantics() {
    assert_eq!(scan_int("42"), 42);
    assert_eq!(scan_int(" 42"), 42);
    assert_eq!(scan_int("+7"), 7);
    assert_eq!(scan_int("-3"), -3);
    assert_eq!(scan_int("12x"), 12);
    assert_eq!(scan_int("abc"), 0);
    assert_eq!(scan_int(""), 0);
    assert_eq!(scan_int("999999999999999999999"), 0);
}

#[test]
fn parse_with_consumes_a_value_and_leaves_everything_else_positional() {
    let mut seen: Vec<String> = Vec::new();
    let rest = parse_with(&args(&["--app", "x", "pos", "--flag"]), |arg, next| {
        seen.push(arg.to_string());
        match (arg, next) {
            ("--app", Some(_)) => Some(Take::Value),
            ("--flag", _) => Some(Take::Rest),
            _ => None,
        }
    });
    assert_eq!(rest, args(&["pos"]));
    assert_eq!(seen, args(&["--app", "pos", "--flag"]));
}

#[test]
fn parse_with_keeps_a_valueless_flag_as_positional() {
    // No value follows, so the flag is never recognised: that is what makes
    // an unfinished `--timeout` fall through to the positional check.
    let rest = parse_with(&args(&["--output"]), |arg, next| match (arg, next) {
        ("--output", Some(_)) => Some(Take::Value),
        _ => None,
    });
    assert_eq!(rest, args(&["--output"]));
}

#[test]
fn take_common_records_each_shared_flag() {
    let mut flags = WorkflowFlags::default();
    assert_eq!(
        flags.take_common("--config", Some("/tmp/w.yaml")),
        Some(Take::Value)
    );
    assert_eq!(flags.take_common("--dry-run", None), Some(Take::Rest));
    assert_eq!(flags.take_common("--verbose", None), Some(Take::Rest));
    assert_eq!(
        flags.take_common("--output", Some("json")),
        Some(Take::Value)
    );
    assert_eq!(flags.take_common("--other", None), None);
    assert_eq!(
        flags,
        WorkflowFlags {
            config_path: "/tmp/w.yaml".to_string(),
            dry_run: true,
            output: "json".to_string(),
            verbose: true,
        }
    );
}

#[test]
fn parse_common_reads_values_and_keeps_positionals() {
    let (flags, rest) = parse_common(&args(&[
        "--output",
        "json",
        "--dry-run",
        "--verbose",
        "--config",
        "a.yaml",
        "extra",
    ]))
    .unwrap();
    assert_eq!(flags.output, "json");
    assert_eq!(flags.config_path, "a.yaml");
    assert!(flags.dry_run && flags.verbose);
    assert_eq!(rest, args(&["extra"]));
}

#[test]
fn parse_common_defaults_are_text_and_no_path() {
    let (flags, rest) = parse_common(&[]).unwrap();
    assert_eq!(flags.output, "text");
    assert_eq!(flags.config_path, "");
    assert!(!flags.dry_run && !flags.verbose);
    assert!(rest.is_empty());
}

#[test]
fn parse_common_rejects_an_unknown_flag_the_go_way() {
    let error = parse_common(&args(&["--bogus"])).unwrap_err();
    assert_eq!(error, "flag provided but not defined: -bogus");
}

#[test]
fn parse_common_treats_dash_h_as_go_help() {
    let error = parse_common(&args(&["-h"])).unwrap_err();
    assert_eq!(error, "flag: help requested");
}

#[test]
fn parse_common_reports_a_missing_value() {
    let error = parse_common(&args(&["--output"])).unwrap_err();
    assert_eq!(error, "flag needs an argument: -output");
}
