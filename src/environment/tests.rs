// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the environment rules.

use super::*;

#[test]
fn a_pinned_mode_has_to_match_the_run() {
    assert!(check_with(Mode::Ci, Some(Mode::Ci), false, None, true).is_ok());
    let error = check_with(Mode::Dev, Some(Mode::Ci), false, None, true).unwrap_err();
    assert!(error.contains("requires --mode ci"), "{error}");
    // A command line that names no mode is not a contradiction.
    assert!(check_with(Mode::Ci, Some(Mode::Dev), false, None, false).is_ok());
}

#[test]
fn ci_refuses_the_developer_mode_unless_the_pipeline_pinned_it() {
    let error = check_with(Mode::Dev, None, true, None, true).unwrap_err();
    assert!(error.contains("CI runs the authoritative gate"), "{error}");
    // A pipeline that declared the developer mode may run it.
    assert!(check_with(Mode::Dev, Some(Mode::Dev), true, None, true).is_ok());
}

#[test]
fn ci_refuses_the_fast_driver() {
    let error = check_with(Mode::Ci, None, true, Some("blaze".to_string()), true).unwrap_err();
    assert!(error.contains("answers with cargo"), "{error}");
    assert!(check_with(Mode::Ci, None, true, Some("cargo".to_string()), true).is_ok());
}

#[test]
fn a_developer_machine_may_run_either_mode() {
    assert!(check_with(Mode::Dev, None, false, Some("blaze".to_string()), true).is_ok());
    assert!(check_with(Mode::Ci, None, false, Some("blaze".to_string()), true).is_ok());
}

#[test]
fn reads_the_pinned_mode() {
    assert_eq!(parse_required_mode("ci").unwrap(), Some(Mode::Ci));
    assert_eq!(parse_required_mode("CI").unwrap(), Some(Mode::Ci));
    assert_eq!(parse_required_mode("release").unwrap(), Some(Mode::Ci));
    assert_eq!(parse_required_mode("dev").unwrap(), Some(Mode::Dev));
    assert_eq!(parse_required_mode("  ").unwrap(), None);
    let error = parse_required_mode("quick").unwrap_err();
    assert!(error.contains("must be `ci` or `dev`"), "{error}");
}

#[test]
fn reads_the_requested_driver() {
    assert_eq!(parse_driver("cargo").unwrap(), Some("cargo".to_string()));
    assert_eq!(parse_driver("BLAZE").unwrap(), Some("blaze".to_string()));
    assert_eq!(parse_driver("").unwrap(), None);
    let error = parse_driver("turbo").unwrap_err();
    assert!(error.contains("must be `cargo` or `blaze`"), "{error}");
}

#[test]
fn an_empty_variable_is_not_set() {
    assert!(!is_set(None));
    assert!(!is_set(Some("")));
    assert!(!is_set(Some("0")));
    assert!(!is_set(Some("false")));
    assert!(is_set(Some("true")));
    assert!(is_set(Some("1")));
}
