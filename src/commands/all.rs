// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `all` subcommand: the whole quality gate in one run.
//!
//! The order mirrors the Go original: install, lint, test, build, coverage,
//! then the static gates. Any step that fails stops the run and the command
//! exits non-zero — there is no way to bypass a failing gate.
//!
//! A Rust project has no Go toolchain, so the aggregate runs the Rust gates
//! in their place and keeps the same static gates afterwards.

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

/// The static gates every project runs.
fn static_steps() -> Vec<Step> {
    vec![
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

/// The steps `all` runs, in order.
///
/// The toolchain steps depend on the language of the project.
pub fn steps(cfg: &Config) -> Vec<Step> {
    let mut steps = if is_rust_project(cfg) {
        vec![Step {
            name: "rust",
            required: true,
        }]
    } else {
        let mut names = vec!["install", "lint", "test", "build"];
        if !cfg.is_dev() {
            names.push("coverage");
        }
        names
            .into_iter()
            .map(|name| Step {
                name,
                required: true,
            })
            .collect()
    };
    steps.extend(static_steps());
    steps
}

/// Whether the project is a Rust project without a Go module.
fn is_rust_project(cfg: &Config) -> bool {
    rustgate::is_rust_project(cfg.root()) && !cfg.root().join("go.mod").exists()
}

/// Runs the aggregate gate and returns the exit code.
pub fn run(cfg: &Config) -> Result<i32, String> {
    announce(cfg);
    let mut failures: Vec<String> = Vec::new();
    for step in steps(cfg) {
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

/// Says which environment this run belongs to, and what it therefore leaves
/// out.
///
/// The warning is the point: a `dev` run is quick because it is narrower, and
/// a narrower run must never be mistaken for the verdict `ci` gives.
fn announce(cfg: &Config) {
    if !cfg.is_dev() {
        println!(
            "🔒 ci environment — whole workspace, cargo, every gate{}",
            if crate::environment::in_ci() {
                " (CI detected: the mode cannot be lowered here)"
            } else {
                ""
            }
        );
        return;
    }
    println!(
        "⚡ dev environment — changed crates only (since {}), fast build driver, \
         coverage and rustdoc/audit skipped",
        cfg.base_branch
    );
    println!("   This is a smoke test; run the same command with --mode ci before releasing.");
    if crate::environment::in_ci() {
        println!(
            "   ⚠ CI job: this job declared the developer's mode, so it is a smoke \
             test, not the verdict — the `full` job is the gate."
        );
    }
}

/// Runs one step of the aggregate gate.
fn run_step(cfg: &Config, name: &str) -> Result<i32, String> {
    match name {
        "install" => crate::commands::toolchain::install(cfg),
        "lint" => crate::commands::toolchain::lint(cfg),
        "test" => crate::commands::toolchain::test(cfg),
        "build" => crate::commands::toolchain::build(cfg),
        "coverage" => crate::commands::toolchain::coverage(cfg),
        "rust" => run_rust(cfg),
        other => {
            let result = run_gate(cfg, other)?;
            Ok(gates::emit(cfg, &result))
        }
    }
}

/// Runs one static gate.
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
