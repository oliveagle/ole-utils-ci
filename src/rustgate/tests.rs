// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the Rust gates.

use super::*;

#[test]
fn compares_versions() {
    assert!(version_gt("1.85.0", "1.84.9"));
    assert!(!version_gt("1.85.0", "1.85.0"));
    assert!(!version_gt("1.84.9", "1.85.0"));
}

#[test]
fn lists_every_rust_gate() {
    assert_eq!(GATES.len(), 10);
    assert!(GATES.contains(&"rust-clippy"));
}

#[test]
fn dev_leaves_out_only_the_slow_gates() {
    assert!(skipped_in_dev("rust-doc").is_some());
    assert!(skipped_in_dev("rust-audit").is_some());
    assert!(skipped_in_dev("rust-test").is_none());
    assert!(skipped_in_dev("rust-clippy").is_none());
}

#[test]
fn a_skipped_gate_says_so_instead_of_looking_like_a_pass() {
    let mut cfg = Config::new();
    cfg.mode = crate::config::Mode::Dev;
    let result = skipped(&cfg, "rust-doc", "ci runs it");
    assert!(result.passed());
    assert_eq!(result.report.summary, "skipped — ci runs it");
    let extra = result.report.extra.expect("extra");
    assert_eq!(extra["skipped"], serde_json::json!(true));
    assert_eq!(extra["mode"], serde_json::json!("dev"));
}
