// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Executes the declared cases through the sandboxed Rhai engine.

use std::time::Instant;

use crate::output::{Diagnostic, Report};

use super::cases::Case;
use super::engine::{self, LoadedSource, RuntimeCoverage};
use super::flags::RhaiFlags;

/// Runtime test results for one file.
pub(super) struct Execution {
    pub(super) coverage: RuntimeCoverage,
    pub(super) case_results: Vec<serde_json::Value>,
    pub(super) faults: usize,
}

/// Calls every case for a file and records faults, expected values and time.
pub(super) fn execute_cases(
    loaded: &LoadedSource,
    file_cases: &[&Case],
    flags: &RhaiFlags,
    rel: &str,
    report: &mut Report,
) -> Execution {
    let coverage = RuntimeCoverage::shared();
    let mut case_results = Vec::new();
    let mut faults = 0usize;
    match engine::coverage_engine(&coverage, flags.max_operations.max(1) as u64) {
        Ok(engine) => {
            for case in file_cases {
                let (result, case_faults) = run_case(&engine, loaded, case, flags, rel, report);
                case_results.push(result);
                faults += case_faults;
            }
        }
        Err(error) => {
            report.add(Diagnostic::error(rel, "rhai/engine", error));
            faults += 1;
        }
    }

    let state = coverage.lock().map(|state| state.clone());
    Execution {
        coverage: state.unwrap_or_default(),
        case_results,
        faults,
    }
}

/// Runs one case and returns its payload plus how many faults it raised.
fn run_case(
    engine: &rhai::Engine,
    loaded: &LoadedSource,
    case: &Case,
    flags: &RhaiFlags,
    rel: &str,
    report: &mut Report,
) -> (serde_json::Value, usize) {
    let name = case.display_name(rel);
    let line = loaded.branches.first().map_or(0, |branch| branch.line);
    let mut result = serde_json::json!({ "name": name });
    let started = Instant::now();
    let faults = match engine::call_hook(engine, &loaded.runtime_ast, &case.function, &case.args) {
        Ok(value) => {
            let elapsed = started.elapsed();
            let faults = record_result(&value, case, rel, &name, line, &mut result, report);
            faults + record_timeout(flags, elapsed, rel, &name, line, &mut result, report)
        }
        Err(error) => {
            report.add(
                Diagnostic::error(rel, "rhai/test", format!("case {name}: {error}")).at_line(line),
            );
            result["error"] = serde_json::Value::String(error);
            result["ok"] = serde_json::Value::Bool(false);
            1
        }
    };
    if faults == 0 {
        result["ok"] = serde_json::Value::Bool(true);
    }
    (result, faults)
}

/// Stores the JSON result and checks it against the case's expectation.
fn record_result(
    value: &rhai::Dynamic,
    case: &Case,
    rel: &str,
    name: &str,
    line: usize,
    result: &mut serde_json::Value,
    report: &mut Report,
) -> usize {
    match engine::to_json(value) {
        Ok(json) => {
            result["result"] = json.clone();
            record_expectation(&json, case, rel, name, line, result, report)
        }
        Err(message) => {
            report.add(Diagnostic::error(rel, "rhai/result", message));
            result["ok"] = serde_json::Value::Bool(false);
            1
        }
    }
}

/// Compares the result against `expected` when the case declares one.
fn record_expectation(
    json: &serde_json::Value,
    case: &Case,
    rel: &str,
    name: &str,
    line: usize,
    result: &mut serde_json::Value,
    report: &mut Report,
) -> usize {
    let Some(expected) = &case.expected else {
        return 0;
    };
    if json == expected {
        return 0;
    }
    report.add(
        Diagnostic::error(
            rel,
            "rhai/expected",
            format!("case {name} returned a different result"),
        )
        .at_line(line),
    );
    result["ok"] = serde_json::Value::Bool(false);
    1
}

/// Adds a diagnostic when the case exceeded `--timeout`.
fn record_timeout(
    flags: &RhaiFlags,
    elapsed: std::time::Duration,
    rel: &str,
    name: &str,
    line: usize,
    result: &mut serde_json::Value,
    report: &mut Report,
) -> usize {
    if flags.timeout_ns <= 0 || elapsed.as_nanos() as i64 <= flags.timeout_ns {
        return 0;
    }
    report.add(
        Diagnostic::error(
            rel,
            "rhai/timeout",
            format!("case {name} took longer than its budget"),
        )
        .at_line(line),
    );
    result["ok"] = serde_json::Value::Bool(false);
    1
}
