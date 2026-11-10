// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `all` subcommand: the whole quality gate in one run.
//!
//! The order mirrors the Go original: install, lint, test, build, coverage,
//! then the static gates. Any step that fails stops the run and the command
//! exits non-zero — there is no way to bypass a failing gate.

use crate::config::Config;
use crate::gates;
use crate::rustgate;

/// One step of the aggregate gate.
pub struct Step {
    /// The name shown while the step runs.
    pub name: &'static str,
    /// Whether the step is required to pass.
    pub required: bool,
}

/// The steps `all` runs, in order.
pub fn steps() -> Vec<Step> {
    vec![
        Step {
            name: "install",
            required: true,
        },
        Step {
            name: "lint",
            required: true,
        },
        Step {
            name: "test",
            required: true,
        },
        Step {
            name: "build",
            required: true,
        },
        Step {
            name: "coverage",
            required: true,
        },
        Step {
            name: "sanity",
            required: true,
        },
        Step {
            name: "complexity",
            required: true,
        },
        Step {
            name: "file-length",
            required: true,
        },
        Step {
            name: "license",
            required: true,
        },
        Step {
            name: "structure",
            required: true,
        },
        Step {
            name: "bugs",
            required: true,
        },
    ]
}

/// Runs the aggregate gate and returns the exit code.
pub fn run(cfg: &Config) -> Result<i32, String> {
    let mut failures: Vec<String> = Vec::new();
    for step in steps() {
        println!("── {} ──", step.name);
        let outcome = run_step(cfg, step.name);
        match outcome {
            Ok(0) => println!("✅ {} passed", step.name),
            Ok(code) => {
                if step.required {
                    failures.push(step.name.to_string());
                    println!("✗ {} failed (exit {code})", step.name);
                    break;
                }
            }
            Err(e) => {
                println!("✗ {} failed: {e}", step.name);
                if step.required {
                    failures.push(step.name.to_string());
                    break;
                }
            }
        }
    }
    if failures.is_empty() {
        println!("✅ All quality gates passed");
        Ok(0)
    } else {
        Err(format!("quality gate failed: {}", failures.join(", ")))
    }
}

fn run_step(cfg: &Config, name: &str) -> Result<i32, String> {
    match name {
        "install" => crate::commands::toolchain::install(cfg),
        "lint" => crate::commands::toolchain::lint(cfg),
        "test" => crate::commands::toolchain::test(cfg),
        "build" => crate::commands::toolchain::build(cfg),
        "coverage" => crate::commands::toolchain::coverage(cfg),
        other => {
            let result = run_gate(cfg, other)?;
            Ok(gates::emit(cfg, &result))
        }
    }
}

fn run_gate(cfg: &Config, name: &str) -> Result<gates::GateResult, String> {
    match name {
        "sanity" => gates::sanity::run(cfg),
        "complexity" => gates::complexity::run(cfg),
        "file-length" => gates::file_length::run(cfg),
        "license" => gates::license::run(cfg),
        "structure" => gates::structure::run(cfg),
        "bugs" => gates::bugs::run(cfg),
        other => Err(format!("unknown gate: {other}")),
    }
}

/// Runs every Rust gate and returns the exit code.
pub fn run_rust(cfg: &Config) -> Result<i32, String> {
    if !rustgate::is_rust_project(cfg.root()) {
        println!("No Cargo.toml — skipping the Rust gates.");
        return Ok(0);
    }
    let results = rustgate::run_all(cfg)?;
    let mut failures = 0;
    for (name, result) in &results {
        let mark = if result.passed() { "✅" } else { "✗" };
        println!("  {mark} {name}: {}", result.report.summary);
        if !result.passed() {
            failures += 1;
            print!("{}", result.report.render_text());
        }
    }
    if failures == 0 {
        println!("✅ All Rust gates passed");
        Ok(0)
    } else {
        Err(format!("{failures} Rust gate(s) failed"))
    }
}
