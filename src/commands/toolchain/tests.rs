// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the Go toolchain subcommands.

use super::*;

#[test]
fn builds_a_junit_document() {
    let json = "{\"Action\":\"pass\",\"Package\":\"pkg\",\"Test\":\"TestOne\"}\n";
    let xml = junit::junit_from_go_json(json);
    assert!(xml.contains("<testsuite name=\"pkg\""));
    assert!(xml.contains("name=\"TestOne\""));
}

#[test]
fn ignores_unparsable_lines() {
    let xml = junit::junit_from_go_json("not json\n");
    assert!(xml.contains("<testsuites "));
    assert!(!xml.contains("testcase"));
}

#[test]
fn the_coverage_floor_is_the_configured_one() {
    assert_eq!(coverage_floor(), crate::config::limits::COVERAGE_FLOOR);
}
