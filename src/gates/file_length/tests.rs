// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the file-length gate.

use super::*;

#[test]
fn maps_extensions_to_profiles() {
    assert_eq!(profile_for_extension(".rs"), Some("source"));
    assert_eq!(profile_for_extension(".sh"), Some("script"));
    assert_eq!(profile_for_extension(".html"), Some("html"));
    assert_eq!(profile_for_extension(".yaml"), Some("declarative"));
    assert_eq!(profile_for_extension(".bin"), None);
}

#[test]
fn built_in_budgets_are_the_documented_ones() {
    let p = default_profiles();
    assert_eq!(p["source"].max, 500);
    assert_eq!(p["source"].warn, 400);
    assert_eq!(p["html"].max, 1200);
    assert_eq!(p["declarative"].max, 1000);
}

#[test]
fn a_file_over_its_budget_is_an_error_not_a_warning() {
    // 420 lines sits inside the 400/500 source band. That band used to report a
    // warning and leave the exit code at zero; a budget is a requirement now.
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("big.rs"), "fn f() {}\n".repeat(420)).expect("write");
    let mut cfg = Config::default();
    cfg.project_root = dir.path().to_path_buf();

    let violations = scan(&cfg, &QualityConfig::default());
    assert_eq!(violations.len(), 1, "the file is over the 400-line budget");
    assert_eq!(violations[0].lines, 420);
    assert_eq!(
        violations[0].severity,
        Severity::Error,
        "over budget must fail the gate, not warn about it"
    );
}
