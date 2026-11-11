// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the ALIGN verdict.

use super::*;

#[test]
fn a_release_tag_names_the_release_branch() {
    assert_eq!(release_branch_name("v0.2.9"), "release");
    assert_eq!(release_branch_name("v1.0"), "release");
    assert_eq!(release_branch_name("v1.2.3-rc1"), "release");
}

#[test]
fn a_tag_without_a_dot_names_no_branch() {
    assert_eq!(release_branch_name("v1"), "");
    assert_eq!(release_branch_name("v"), "");
    assert_eq!(release_branch_name(""), "");
    assert_eq!(release_branch_name("1.2.3"), "");
}

#[test]
fn a_tag_with_junk_names_no_branch() {
    assert_eq!(release_branch_name("v1.x"), "");
    assert_eq!(release_branch_name("v-1.2"), "");
    assert_eq!(release_branch_name("release-1.2"), "");
}

#[test]
fn a_missing_tag_verdict_is_empty() {
    assert_eq!(classify_align("abcdef12", "", ""), AlignKind::Empty);
}

#[test]
fn main_on_the_tag_is_the_simple_model() {
    assert_eq!(classify_align("abcdef12", "", "abcdef12"), AlignKind::Two);
}

#[test]
fn main_and_release_on_the_tag_is_the_strict_model() {
    assert_eq!(
        classify_align("abcdef12", "abcdef12", "abcdef12"),
        AlignKind::Three
    );
}

#[test]
fn a_release_branch_that_disagrees_is_drift() {
    assert_eq!(
        classify_align("abcdef12", "99999999", "abcdef12"),
        AlignKind::Red
    );
}

#[test]
fn a_main_that_disagrees_is_drift() {
    assert_eq!(classify_align("99999999", "", "abcdef12"), AlignKind::Red);
}

#[test]
fn the_comparison_runs_on_the_short_sha() {
    assert_eq!(
        classify_align("abcdef1234567890", "", "abcdef1299999999"),
        AlignKind::Two
    );
}
