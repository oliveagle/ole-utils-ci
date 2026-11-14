// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The CI pipelines must state the mode they run in.
//!
//! The rule the tool enforces is "the mode of a CI job cannot be degraded".
//! This test enforces the other half: the pipeline file must say which mode
//! every job runs in, so the rule has a place to be read and cannot be
//! dropped by accident.

use std::collections::BTreeMap;

use serde_yaml::Value;

/// The pipeline file of this repository.
const WORKFLOW: &str = ".github/workflows/ci.yml";

/// The environment variable that pins the mode of a job.
const REQUIRE_MODE: &str = "DEVINE_UTILS_REQUIRE_MODE";

/// The environment variable that names the build driver.
const DRIVER: &str = "DEVINE_UTILS_RUST_DRIVER";

/// Reads the workflow and returns its jobs.
fn jobs() -> BTreeMap<String, Value> {
    let text = std::fs::read_to_string(WORKFLOW).expect("the workflow file exists");
    let workflow: Value = serde_yaml::from_str(&text).expect("the workflow is valid YAML");
    let jobs = workflow
        .get("jobs")
        .and_then(Value::as_mapping)
        .expect("the workflow has jobs");
    jobs.iter()
        .filter_map(|(name, job)| {
            let name = name.as_str()?.to_string();
            Some((name, job.clone()))
        })
        .collect()
}

/// The mode a job pins.
fn pinned_mode(job: &Value) -> Option<String> {
    job.get("env")
        .and_then(Value::as_mapping)
        .and_then(|env| env.get(REQUIRE_MODE))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Every `run:` command of a job.
fn run_commands(job: &Value) -> Vec<String> {
    let mut commands = Vec::new();
    if let Some(steps) = job.get("steps").and_then(Value::as_sequence) {
        for step in steps {
            if let Some(run) = step.get("run").and_then(Value::as_str) {
                commands.push(run.to_string());
            }
        }
    }
    commands
}

#[test]
fn every_job_pins_the_mode_it_runs_in() {
    let jobs = jobs();
    assert!(!jobs.is_empty(), "the workflow has no jobs");
    for (name, job) in &jobs {
        let mode =
            pinned_mode(job).unwrap_or_else(|| panic!("job `{name}` does not pin {REQUIRE_MODE}"));
        assert!(
            mode == "ci" || mode == "dev",
            "job `{name}` pins the unknown mode `{mode}`"
        );
    }
}

#[test]
fn the_full_job_runs_the_authoritative_mode() {
    let jobs = jobs();
    let full = jobs
        .get("full")
        .expect("the workflow has a `full` job — the gate itself");
    assert_eq!(
        pinned_mode(full).as_deref(),
        Some("ci"),
        "the `full` job must run the authoritative mode"
    );
}

#[test]
fn no_job_asks_for_the_fast_driver() {
    for (name, job) in jobs() {
        let driver = job
            .get("env")
            .and_then(Value::as_mapping)
            .and_then(|env| env.get(DRIVER))
            .and_then(Value::as_str);
        assert!(
            driver.is_none(),
            "job `{name}` sets {DRIVER}: a CI job must not answer from the fast driver"
        );
    }
}

#[test]
fn every_command_asks_for_the_mode_its_job_pinned() {
    for (name, job) in jobs() {
        let pinned = pinned_mode(&job).expect("the job pins a mode");
        for command in run_commands(&job) {
            for asked in modes_in(&command) {
                assert_eq!(
                    asked, pinned,
                    "job `{name}` pins `{pinned}` but runs `--mode {asked}`"
                );
            }
        }
    }
}

#[test]
fn the_workflow_states_the_rule_it_enforces() {
    let text = std::fs::read_to_string(WORKFLOW).expect("the workflow file exists");
    assert!(
        text.contains(REQUIRE_MODE),
        "the workflow must name the variable that pins the mode"
    );
    assert!(
        text.contains("cannot be lowered") || text.contains("cannot be degraded"),
        "the workflow must say that the mode cannot be lowered"
    );
}

/// The modes a command asks for, in the order they appear.
fn modes_in(command: &str) -> Vec<String> {
    let mut modes = Vec::new();
    let mut rest = command;
    while let Some(index) = rest.find("--mode") {
        rest = &rest[index + "--mode".len()..];
        let trimmed = rest.trim_start();
        if let Some(value) = trimmed.strip_prefix('=') {
            modes.push(value.trim().to_string());
            rest = &value[value.len()..];
        } else {
            let value: String = trimmed.chars().take_while(|c| !c.is_whitespace()).collect();
            rest = &trimmed[value.len()..];
            modes.push(value);
        }
    }
    modes
}
