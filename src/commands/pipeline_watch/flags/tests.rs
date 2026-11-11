// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the `pipeline-watch` flag parser.

use super::*;

/// `&["a", "b"]` -> `["a".to_string(), "b".to_string()]`.
fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn reads_the_known_flags() {
    let flags = parse_flags(&args(&[
        "--config",
        "/tmp/cfg.yaml",
        "--interval",
        "30",
        "--per-repo",
        "9",
        "--base-url",
        "https://example.test",
        "--json",
    ]));
    assert_eq!(flags.config, "/tmp/cfg.yaml");
    assert_eq!(flags.interval, 30);
    assert_eq!(flags.per_repo, 9);
    assert_eq!(flags.base_url, "https://example.test");
    assert!(flags.json);
}

#[test]
fn ignores_unknown_flags() {
    let flags = parse_flags(&args(&["--nope", "value", "--json"]));
    assert!(flags.json);
    assert_eq!(flags.config, "");
}

#[test]
fn a_flag_without_a_value_keeps_the_default() {
    let flags = parse_flags(&args(&["--config"]));
    assert_eq!(flags.config, "");
}

#[test]
fn a_non_numeric_interval_is_ignored() {
    let flags = parse_flags(&args(&["--interval", "abc", "--per-repo", "-3"]));
    assert_eq!(flags.interval, 0);
    assert_eq!(flags.per_repo, 0);
}

#[test]
fn atoi_reads_plain_digits_only() {
    assert_eq!(atoi_or_zero("0"), 0);
    assert_eq!(atoi_or_zero("12"), 12);
    assert_eq!(atoi_or_zero(""), 0);
    assert_eq!(atoi_or_zero("1.5"), 0);
    assert_eq!(atoi_or_zero("-1"), 0);
    assert_eq!(atoi_or_zero(" 1"), 0);
}
