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
    } else if crate::commands::mojo::is_project(cfg.root()) {
        vec![Step {
            name: "mojo",
            required: true,
        }]
    } else if crate::commands::python::is_project(cfg.root()) {
        vec![Step {
            name: "python",
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
        // Reached by name from the bare aggregate, where no language flag was parsed: there is
        // nothing to thread through.
        "python" => run_python(cfg, &crate::commands::python::Options::default()),
        "mojo" => run_mojo(cfg, &crate::commands::mojo::Options::default()),
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

/// Runs every Python gate and returns the exit code.
///
/// `opts` is the caller's own command line: `python all --tool ./venv/bin/ruff` means every gate in
/// the group, not every gate except the one that needed the flag.
pub fn run_python(cfg: &Config, opts: &crate::commands::python::Options) -> Result<i32, String> {
    if !opts.tool.is_empty() {
        // The group drives three different programs. One flag naming one of them would hand it to
        // all three, and the run would fail with the linter's usage banner — the kind of failure
        // that reads like a broken project rather than a misunderstood flag.
        return Err(
            "--tool names one binary, and the Python group drives three (ruff, mypy, pytest). \
             Name them one by one instead: --ruff, --mypy, --interpreter."
                .to_string(),
        );
    }
    if !crate::commands::python::is_project(cfg.root()) {
        println!("No pyproject.toml, setup.py or requirements.txt — skipping the Python gates.");
        return Ok(0);
    }
    run_gates(cfg, "Python", &crate::commands::python::STEPS, |name| {
        crate::commands::python::run(name, cfg, opts)
    })
}

/// Runs every Mojo gate and returns the exit code.
///
/// `opts` is the caller's own command line, as for `run_python`.
pub fn run_mojo(cfg: &Config, opts: &crate::commands::mojo::Options) -> Result<i32, String> {
    if !crate::commands::mojo::is_project(cfg.root()) {
        println!("No Mojo sources — skipping the Mojo gates.");
        return Ok(0);
    }
    run_gates(cfg, "Mojo", &crate::commands::mojo::STEPS, |name| {
        crate::commands::mojo::run(name, cfg, opts)
    })
}

/// Runs a language's gates in order, stopping at the first failure.
///
/// The first failure stops the run, exactly like all itself: a later gate
/// cannot make an earlier one pass.
fn run_gates<F>(_cfg: &Config, title: &str, steps: &[&str], mut run: F) -> Result<i32, String>
where
    F: FnMut(&str) -> Result<i32, String>,
{
    for name in steps {
        println!("── {name} ──");
        match run(name) {
            Ok(0) => println!("✅ {name} passed"),
            Ok(code) => {
                println!("✗ {name} failed (exit {code})");
                return Err(format!("{title} gate failed: {name}"));
            }
            Err(e) => {
                println!("✗ {name} failed: {e}");
                return Err(format!("{title} gate failed: {name}"));
            }
        }
    }
    println!("✅ All {title} gates passed");
    Ok(0)
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
