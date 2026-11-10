// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the `flag`-style argument parser.

use super::*;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn parses_long_and_short_forms() {
    let mut fs = FlagSet::new();
    fs.str_flag("project-root", ".")
        .bool_flag("verbose", false)
        .alias("v", "verbose")
        .int_flag("max-file-lines", 500);
    fs.parse(&args(&[
        "--project-root",
        "/tmp",
        "-v",
        "--max-file-lines=42",
    ]))
    .unwrap();
    assert_eq!(fs.get("project-root").unwrap().as_str(""), "/tmp");
    assert!(fs.get("verbose").unwrap().as_bool(false));
    assert_eq!(fs.get("max-file-lines").unwrap().as_int(0), 42);
}

#[test]
fn collects_positional_arguments() {
    let mut fs = FlagSet::new();
    fs.bool_flag("verbose", false).alias("v", "verbose");
    fs.parse(&args(&["-v", "bump", "patch"])).unwrap();
    assert_eq!(fs.args, vec!["bump".to_string(), "patch".to_string()]);
}

#[test]
fn rejects_unknown_flags() {
    let mut fs = FlagSet::new();
    fs.bool_flag("verbose", false);
    let err = fs.parse(&args(&["--nope"])).unwrap_err();
    assert!(matches!(err, ParseError::UnknownFlag { .. }));
}

#[test]
fn reports_missing_values() {
    let mut fs = FlagSet::new();
    fs.str_flag("out", "");
    let err = fs.parse(&args(&["--out"])).unwrap_err();
    assert!(matches!(err, ParseError::MissingValue { .. }));
}

#[test]
fn rejects_non_numeric_integers() {
    let mut fs = FlagSet::new();
    fs.int_flag("n", 1);
    let err = fs.parse(&args(&["--n=abc"])).unwrap_err();
    assert!(matches!(err, ParseError::InvalidValue { .. }));
}

#[test]
fn splits_comma_separated_lists() {
    let mut fs = FlagSet::new();
    fs.list_flag("coverage-allowlist", &[]);
    fs.parse(&args(&["--coverage-allowlist=a,b , c"])).unwrap();
    // Go's coverageAllowlistFlag.Set is strings.Split(value, ",") — no trim.
    assert_eq!(
        fs.get("coverage-allowlist").unwrap().as_list(),
        ["a", "b ", " c"]
    );
}

#[test]
fn empty_list_value_keeps_the_previous_one() {
    let mut fs = FlagSet::new();
    fs.list_flag("coverage-allowlist", &[]);
    fs.parse(&args(&[
        "--coverage-allowlist=a,b",
        "--coverage-allowlist=",
    ]))
    .unwrap();
    assert_eq!(fs.get("coverage-allowlist").unwrap().as_list(), ["a", "b"]);
}

#[test]
fn stops_at_the_first_positional() {
    let mut fs = FlagSet::new();
    fs.bool_flag("verbose", false);
    fs.parse(&args(&["patch", "--verbose"])).unwrap();
    assert!(!fs.get("verbose").unwrap().as_bool(false));
    assert_eq!(fs.args, vec!["patch".to_string(), "--verbose".to_string()]);
}

#[test]
fn a_lone_dash_stops_parsing_too() {
    let mut fs = FlagSet::new();
    fs.bool_flag("verbose", false);
    fs.parse(&args(&["-", "--verbose"])).unwrap();
    assert!(!fs.get("verbose").unwrap().as_bool(false));
    assert_eq!(fs.args, vec!["-".to_string(), "--verbose".to_string()]);
}

#[test]
fn help_spellings_are_reported_as_help_requested() {
    let mut fs = FlagSet::new();
    fs.bool_flag("verbose", false);
    for spelling in ["-h", "--help", "-help", "--help=1"] {
        let err = fs.parse(&args(&[spelling])).unwrap_err();
        assert!(matches!(err, ParseError::Help), "{spelling}: {err}");
    }
}

#[test]
fn malformed_flag_tokens_report_bad_syntax() {
    let mut fs = FlagSet::new();
    for spelling in ["---foo", "--=x", "-=x"] {
        let err = fs.parse(&args(&[spelling])).unwrap_err();
        match err {
            ParseError::BadSyntax { arg } => assert_eq!(arg, spelling),
            other => panic!("{spelling}: unexpected {other}"),
        }
    }
}

#[test]
fn boolean_values_follow_strconv_parse_bool() {
    let mut fs = FlagSet::new();
    fs.bool_flag("verbose", false).alias("v", "verbose");
    fs.parse(&args(&["-v=1"])).unwrap();
    assert!(fs.get("verbose").unwrap().as_bool(false));
    fs.parse(&args(&["--verbose=F"])).unwrap();
    assert!(!fs.get("verbose").unwrap().as_bool(true));
    let err = fs.parse(&args(&["--verbose=maybe"])).unwrap_err();
    assert!(
        matches!(err, ParseError::InvalidBoolValue { ref reason, .. } if reason == "parse error")
    );
}

#[test]
fn integer_values_follow_strconv_parse_int() {
    let mut fs = FlagSet::new();
    fs.int_flag("max-file-lines", 500);
    fs.parse(&args(&["--max-file-lines=0x10"])).unwrap();
    assert_eq!(fs.get("max-file-lines").unwrap().as_int(0), 16);
    let syntax = fs.parse(&args(&["--max-file-lines=abc"])).unwrap_err();
    assert!(
        matches!(syntax, ParseError::InvalidValue { ref reason, .. } if reason == "parse error")
    );
    let range = fs
        .parse(&args(&["--max-file-lines=99999999999999999999"]))
        .unwrap_err();
    assert!(
        matches!(range, ParseError::InvalidValue { ref reason, .. } if reason == "value out of range")
    );
}

#[test]
fn errors_name_the_flag_as_written() {
    let mut fs = FlagSet::new();
    fs.str_flag("output", "").alias("o", "output");
    let err = fs.parse(&args(&["-o"])).unwrap_err();
    assert!(matches!(err, ParseError::MissingValue { ref name } if name == "o"));
    let err = fs.parse(&args(&["-nope"])).unwrap_err();
    assert!(matches!(err, ParseError::UnknownFlag { ref name } if name == "nope"));
}
