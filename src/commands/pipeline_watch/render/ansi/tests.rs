// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the ANSI helpers.

use super::*;

#[test]
fn escape_sequences_do_not_count_towards_the_width() {
    assert_eq!(visible_length("abc"), 3);
    assert_eq!(visible_length("\u{1b}[31mabc\u{1b}[0m"), 3);
    assert_eq!(visible_length("\u{1b}[48;2;46;160;67m01\u{1b}[0m"), 2);
}

#[test]
fn truncation_keeps_the_escape_and_resets() {
    let text = "\u{1b}[31mabcdefgh\u{1b}[0m";
    let cut = truncate_visible(text, 3);
    assert_eq!(visible_length(&cut), 3);
    assert!(cut.ends_with(RESET));
}

#[test]
fn truncation_of_a_short_string_still_resets() {
    let cut = truncate_visible("ab", 5);
    assert_eq!(cut, format!("ab{RESET}"));
}

#[test]
fn the_status_palette_is_shared_by_both_ends() {
    assert_eq!(dot_bg("success"), "\u{1b}[48;2;46;160;67m");
    assert_eq!(dot_fg("success"), "\u{1b}[38;2;46;160;67m");
    assert_eq!(dot_bg("failed"), "\u{1b}[48;2;200;50;50m");
    assert_eq!(dot_fg("failed"), "\u{1b}[38;2;200;50;50m");
    assert_eq!(dot_bg("unknown"), "\u{1b}[48;2;110;110;110m");
}

#[test]
fn the_status_lookup_ignores_case_and_padding() {
    assert_eq!(dot_bg(" Success "), dot_bg("success"));
    assert_eq!(dot_bg("CANCELLED"), dot_bg("canceled"));
}
