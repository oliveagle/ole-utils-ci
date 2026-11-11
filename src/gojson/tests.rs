// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the Go-compatible JSON escaping.

use super::*;

#[test]
fn the_html_significant_bytes_are_escaped() {
    let text = to_go_json(r#"{"a":"x < y & z > w"}"#);
    assert_eq!(text, r#"{"a":"x \u003c y \u0026 z \u003e w"}"#);
}

#[test]
fn the_unicode_line_separators_are_escaped() {
    let text = to_go_json("{\"a\":\"x\u{2028}y\u{2029}z\"}");
    assert_eq!(text, "{\"a\":\"x\\u2028y\\u2029z\"}");
}

#[test]
fn the_backspace_and_form_feed_escapes_match_go() {
    let text = to_go_json(r#"{"a":"x\by\fz"}"#);
    assert_eq!(text, r#"{"a":"x\u0008y\u000cz"}"#);
}

#[test]
fn an_escaped_backslash_is_not_an_escape_introducer() {
    let text = to_go_json(r#"{"a":"x\\by"}"#);
    assert_eq!(text, r#"{"a":"x\\by"}"#);
}

#[test]
fn a_plain_document_is_left_alone() {
    let text = r#"{"a":[1,2,3],"b":"plain"}"#;
    assert_eq!(to_go_json(text), text);
}
