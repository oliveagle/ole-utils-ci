// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the Rhai gate.

mod engine;

use super::cases::Case;
use super::check::run;
use super::flags::RhaiFlags;
use std::path::PathBuf;
use tempfile::tempdir;

fn case(file: &str, function: &str, value: bool) -> Case {
    serde_json::from_value(serde_json::json!({
        "name": format!("{function}-{value}"),
        "file": file,
        "function": function,
        "args": [value],
    }))
    .expect("valid case")
}

#[test]
fn a_fully_covered_script_passes() {
    let dir = tempdir().expect("temporary directory");
    std::fs::write(
        dir.path().join("sample.rhai"),
        "fn choose(x) { if (x) { 1 } else { 2 } }\n",
    )
    .expect("write script");
    let cases = vec![
        case("sample.rhai", "choose", true),
        case("sample.rhai", "choose", false),
    ];
    let flags = RhaiFlags {
        src: dir.path().to_string_lossy().into_owned(),
        cases: write_cases(&dir, &cases).to_string_lossy().into_owned(),
        ..RhaiFlags::default()
    };
    let report = run(&flags).expect("gate runs");
    assert!(report.ok, "{}", report.render_text());
    assert!(report.summary.contains("line 100.0%"), "{}", report.summary);
    assert!(
        report.summary.contains("branch 100.0%"),
        "{}",
        report.summary
    );
}

#[test]
fn a_single_branch_case_fails_branch_coverage() {
    let dir = tempdir().expect("temporary directory");
    std::fs::write(
        dir.path().join("sample.rhai"),
        "fn choose(x) { if (x) { 1 } else { 2 } }\n",
    )
    .expect("write script");
    let cases = vec![case("sample.rhai", "choose", true)];
    let flags = RhaiFlags {
        src: dir.path().to_string_lossy().into_owned(),
        cases: write_cases(&dir, &cases).to_string_lossy().into_owned(),
        ..RhaiFlags::default()
    };
    let report = run(&flags).expect("gate runs");
    assert!(!report.ok);
    assert!(
        report.summary.contains("branch 50.0%"),
        "{}",
        report.summary
    );
}

#[test]
fn a_missing_case_file_is_an_error() {
    let dir = tempdir().expect("temporary directory");
    std::fs::write(dir.path().join("sample.rhai"), "fn one() { 1 }").expect("write script");
    let flags = RhaiFlags {
        src: dir.path().to_string_lossy().into_owned(),
        ..RhaiFlags::default()
    };
    let report = run(&flags).expect("gate runs");
    assert!(!report.ok);
    assert!(
        report.summary.contains("1 test fault(s)"),
        "{}",
        report.summary
    );
}

#[test]
fn an_oversized_source_is_refused_before_parsing() {
    let dir = tempdir().expect("temporary directory");
    std::fs::write(dir.path().join("sample.rhai"), "fn one() { 1 }").expect("write script");
    let flags = RhaiFlags {
        src: dir.path().to_string_lossy().into_owned(),
        max_file_size: 1,
        ..RhaiFlags::default()
    };
    let report = run(&flags).expect("gate runs");
    assert!(!report.ok);
    assert!(report
        .diagnostics
        .iter()
        .any(|d| d.rule == "rhai/file-size"));
}

#[test]
fn complexity_over_budget_fails_the_gate() {
    let dir = tempdir().expect("temporary directory");
    let body = "fn busy(x) { if (x) { 1 } else if (!x) { 2 } else { 3 } }\n";
    std::fs::write(dir.path().join("sample.rhai"), body).expect("write script");
    let cases = vec![
        case("sample.rhai", "busy", true),
        case("sample.rhai", "busy", false),
    ];
    let flags = RhaiFlags {
        src: dir.path().to_string_lossy().into_owned(),
        cases: write_cases(&dir, &cases).to_string_lossy().into_owned(),
        complexity_warn: 2,
        max_complexity: 2,
        ..RhaiFlags::default()
    };
    let report = run(&flags).expect("gate runs");
    assert!(!report.ok);
    assert!(
        report.summary.contains("1 complexity error(s)"),
        "{}",
        report.summary
    );
}

#[test]
fn a_bad_case_script_name_is_reported() {
    let dir = tempdir().expect("temporary directory");
    std::fs::write(dir.path().join("sample.rhai"), "fn one() { 1 }").expect("write script");
    let cases = vec![case("other.rhai", "one", true)];
    let flags = RhaiFlags {
        src: dir.path().to_string_lossy().into_owned(),
        cases: write_cases(&dir, &cases).to_string_lossy().into_owned(),
        ..RhaiFlags::default()
    };
    let report = run(&flags).expect("gate runs");
    assert!(!report.ok);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.rule == "rhai/case-file"),
        "{}",
        report.render_text()
    );
}

fn write_cases(dir: &tempfile::TempDir, cases: &[Case]) -> PathBuf {
    let path = dir.path().join("cases.json");
    serde_json::to_writer_pretty(std::fs::File::create(&path).expect("create cases"), cases)
        .expect("write cases");
    path
}
