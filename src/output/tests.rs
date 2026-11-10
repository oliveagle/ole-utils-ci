// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the report data model and its renderings.

use super::*;

fn sample() -> Report {
    let mut r = Report::new("bugs");
    r.summary = "bugs: 1 marker(s) found".to_string();
    r.add(Diagnostic::error("a.go", "BUG", "bug marker present: BUG").at_line(3)); //nolint:bugs
    r.add(Diagnostic::warning("b.go", "FIXME", "fixme here").at_line(9)); //nolint:bugs
    r
}

#[test]
fn parse_mode_accepts_canonical_values() {
    assert_eq!(parse_mode(""), Mode::Text);
    assert_eq!(parse_mode("TEXT"), Mode::Text);
    assert_eq!(parse_mode("json"), Mode::Json);
    assert_eq!(parse_mode("  toon  "), Mode::Toon);
    assert_eq!(parse_mode("yaml"), Mode::Text);
}

#[test]
fn validate_mode_rejects_unknown_values() {
    for ok in ["", "text", "json", "toon", "TOON"] {
        assert!(validate_mode(ok).is_ok(), "{ok} should be accepted");
    }
    for bad in ["yaml", "csv", "xml"] {
        assert!(validate_mode(bad).is_err(), "{bad} should be rejected");
    }
}

#[test]
fn error_diagnostic_flips_verdict() {
    let r = sample();
    assert!(!r.ok);
    assert_eq!(r.counts.errors, 1);
    assert_eq!(r.counts.warnings, 1);
}

#[test]
fn json_and_toon_expose_the_same_keys() {
    let r = sample();
    let json = serde_json::to_value(&r).unwrap();
    let toon = r.render_toon();
    assert!(json.get("command").is_some());
    assert!(toon.contains("command"));
    assert!(toon.contains("diagnostics"));
    assert!(toon.len() < r.render_json().len());
}

#[test]
fn text_rendering_is_grep_friendly() {
    let text = sample().render_text();
    assert!(text.contains("ERROR  a.go:3  BUG: bug marker present: BUG")); //nolint:bugs
    assert!(text.contains("WARN   b.go:9  FIXME: fixme here")); //nolint:bugs
}
