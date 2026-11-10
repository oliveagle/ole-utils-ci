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
    assert_eq!(
        fs.get("coverage-allowlist").unwrap().as_list(),
        ["a", "b", "c"]
    );
}
