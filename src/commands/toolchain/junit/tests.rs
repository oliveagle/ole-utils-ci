// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the JUnit report.

use super::*;

#[test]
fn a_passing_test_becomes_a_test_case() {
    let json = "{\"Action\":\"pass\",\"Package\":\"pkg\",\"Test\":\"TestOne\",\"Time\":\"2026-01-01T00:00:00Z\",\"Elapsed\":0.015}\n";
    let document = junit_from_go_json(json);
    assert!(document.contains("<testsuite name=\"pkg\" tests=\"1\" failures=\"0\""));
    assert!(document.contains("name=\"TestOne\" time=\"0.015\""));
}

#[test]
fn a_failing_test_carries_a_failure_element() {
    let json = "{\"Action\":\"fail\",\"Package\":\"pkg\",\"Test\":\"TestBad\",\"Time\":\"2026-01-01T00:00:00Z\",\"Elapsed\":0}\n";
    let document = junit_from_go_json(json);
    assert!(document.contains("<failure message=\"Test failed\" type=\"failure\"></failure>"));
    assert!(document.contains("failures=\"1\""));
}

#[test]
fn an_empty_run_still_writes_the_root_element() {
    let document = junit_from_go_json("");
    assert_eq!(
        document,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites tests=\"0\" failures=\"0\" skipped=\"0\" time=\"0\"></testsuites>"
    );
}

#[test]
fn the_package_events_are_skipped() {
    let json = "{\"Action\":\"pass\",\"Package\":\"pkg\",\"Time\":\"2026-01-01T00:00:00Z\",\"Elapsed\":0.1}\n";
    let document = junit_from_go_json(json);
    assert!(!document.contains("testcase"));
}

#[test]
fn the_go_float_spelling_is_kept() {
    assert_eq!(xml::go_g(0.0), "0");
    assert_eq!(xml::go_g(0.015), "0.015");
    assert_eq!(xml::go_g(0.0001), "0.0001");
    assert_eq!(xml::go_g(1e-7), "1e-07");
    assert_eq!(xml::go_g(1_000_000.0), "1e+06");
    assert_eq!(xml::go_g(12345.6789), "12345.6789");
    assert_eq!(xml::go_g(999999.5), "999999.5");
    assert_eq!(xml::go_g(0.3), "0.3");
}

#[test]
fn the_attribute_values_are_escaped() {
    assert_eq!(xml::escape("a\"b"), "a&#34;b");
    assert_eq!(xml::escape("a&b"), "a&amp;b");
    assert_eq!(xml::escape("a<b>c"), "a&lt;b&gt;c");
}
