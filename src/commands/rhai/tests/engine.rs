// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Engine tests for the Rhai parser, instrumentation and sandbox.

use crate::commands::rhai::engine::*;

use tempfile::tempdir;

const BRANCH_SCRIPT: &str = r#"
        fn choose(value) {
            if (value) {
                "yes"
            } else {
                "no"
            }
        }

        fn limit(count) {
            let total = 0;
            let index = 0;
            while index < count {
                total += index;
                index += 1;
            }
            total
        }
    "#;

fn write_script(source: &str) -> tempfile::TempDir {
    let dir = tempdir().expect("temporary directory");
    std::fs::write(dir.path().join("sample.rhai"), source).expect("write script");
    dir
}

#[test]
fn real_rhai_parser_reports_a_positioned_syntax_error() {
    let error = parse("fn broken() { let = ; }").expect_err("broken source");
    assert_eq!(error.rule, "rhai/syntax");
    assert!(error.line > 0, "{error:?}");
}

#[test]
fn executable_lines_come_from_the_ast() {
    let dir = write_script(BRANCH_SCRIPT);
    let loaded = load(&dir.path().join("sample.rhai")).expect("valid script");
    assert!(loaded.lines.contains(&3));
    assert!(loaded.lines.contains(&11));
    assert!(loaded.lines.contains(&13));
    assert_eq!(loaded.branches.len(), 2);
    assert!(loaded.coverage_error.is_none());
}

#[test]
fn switch_case_lines_are_static_targets() {
    let dir = write_script(
        r#"
            fn kind(value) {
                switch value {
                    1 => "one",
                    2 | 3 => "low",
                    _ => "many"
                }
            }
            "#,
    );
    let loaded = load(&dir.path().join("sample.rhai")).expect("valid script");
    assert!(loaded.lines.contains(&3));
    assert!(loaded.lines.contains(&4));
    assert!(loaded.lines.contains(&5));
    assert!(loaded.lines.contains(&6));
}

#[test]
fn branch_probes_observe_both_paths() {
    let dir = write_script(BRANCH_SCRIPT);
    let loaded = load(&dir.path().join("sample.rhai")).expect("valid script");
    let coverage = RuntimeCoverage::shared();
    let engine = coverage_engine(&coverage, 100_000).expect("engine");
    let yes = call_hook(
        &engine,
        &loaded.runtime_ast,
        "choose",
        &[serde_json::json!(true)],
    )
    .expect("true branch");
    let no = call_hook(
        &engine,
        &loaded.runtime_ast,
        "choose",
        &[serde_json::json!(false)],
    )
    .expect("false branch");
    assert_eq!(
        to_json(&yes).expect("JSON result"),
        serde_json::json!("yes")
    );
    assert_eq!(to_json(&no).expect("JSON result"), serde_json::json!("no"));
    let state = coverage.lock().expect("coverage lock");
    assert_eq!(state.branches.len(), 2, "{state:?}");
    assert!(state.lines.contains(&3));
    assert!(state.lines.contains(&4));
    assert!(state.lines.contains(&6));
}

#[test]
fn loops_report_their_guard_outcomes() {
    let dir = write_script(BRANCH_SCRIPT);
    let loaded = load(&dir.path().join("sample.rhai")).expect("valid script");
    let coverage = RuntimeCoverage::shared();
    let engine = coverage_engine(&coverage, 100_000).expect("engine");
    let _ = call_hook(
        &engine,
        &loaded.runtime_ast,
        "limit",
        &[serde_json::json!(2)],
    )
    .expect("loop runs");
    let state = coverage.lock().expect("coverage lock");
    assert_eq!(state.branches.len(), 2, "{state:?}");
    assert!(state.lines.contains(&14));
}

#[test]
fn the_operation_budget_stops_a_runaway_script() {
    let dir = write_script("fn spin() { while true { let value = 1; } }");
    let loaded = load(&dir.path().join("sample.rhai")).expect("valid script");
    let coverage = RuntimeCoverage::shared();
    let engine = coverage_engine(&coverage, 100).expect("engine");
    let error = call_hook(&engine, &loaded.runtime_ast, "spin", &[])
        .expect_err("operation budget stops the loop");
    assert_eq!(error, "Too many operations");
}

#[test]
fn case_scripts_cannot_escape_the_source_root() {
    let dir = write_script("fn one() { 1 }");
    let error = resolve_source(dir.path(), "../outside.rhai").unwrap_err();
    assert!(error.contains("escapes source root"), "{error}");
}
