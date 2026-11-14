// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The Rust-specific quality gates.
//!
//! The Go tool had no notion of Rust; this module adds the gates a Rust
//! repository needs:
//!
//! | command | what it does |
//! |---|---|
//! | `rust-fmt` | `cargo fmt --check` |
//! | `rust-clippy` | `cargo clippy --all-targets -- -D warnings` |
//! | `rust-test` | `cargo test` |
//! | `rust-build` | `cargo build --all-targets` |
//! | `rust-doc` | `cargo doc --no-deps` with warnings denied |
//! | `rust-audit` | dependency advisories and licence audit |
//! | `rust-manifest` | `Cargo.toml` hygiene |
//! | `rust-unsafe` | forbids `unsafe` in production code |
//! | `rust-panic` | forbids `unwrap`/`expect`/`panic!` in production code |
//! | `rust` | all of the above |
//!
//! The flags above are cargo's; [`toolchain::adapt_args`] rewrites them when
//! the driver is [blaze](https://github.com/indexable-inc/blaze).
//!
//! # The two environments
//!
//! [`Mode::Dev`](crate::config::Mode::Dev) narrows the compiling gates to the crates the branch touched
//! ([`changed`]), lets the fast driver answer and leaves the two slow gates
//! (`rust-doc`, `rust-audit`) out. [`Mode::Ci`](crate::config::Mode::Ci) runs the whole workspace,
//! always through cargo, and runs every gate. A `dev` result is a smoke test,
//! never a verdict: each gate it drops says so in its own summary.

pub mod changed;
pub mod manifest;
pub mod scanner;
pub mod scope;
pub mod toolchain;

use std::path::Path;

use crate::config::Config;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};
use scanner::RustFinding;
use scope::{args_of, scoped, whole_workspace};

/// The source-scanning gates: `rust-unsafe` and `rust-panic`.
pub mod source;

/// The manifest gates: `rust-manifest` and `rust-msrv`.
pub mod manifests;

/// The dependency audit gate.
pub mod audit;

pub use audit::audit;
pub use manifests::{installed_rustc_version, manifest_gate, msrv, version_gt};
pub use source::{panic_free, unsafe_code};
/// The names of every Rust gate.
pub const GATES: [&str; 10] = [
    "rust-fmt",
    "rust-clippy",
    "rust-test",
    "rust-build",
    "rust-doc",
    "rust-audit",
    "rust-manifest",
    "rust-unsafe",
    "rust-panic",
    "rust-msrv",
];

/// The gates `dev` leaves out, and why.
const SLOW_GATES: [(&str, &str); 2] = [
    ("rust-doc", "rustdoc rebuilds everything; ci runs it"),
    (
        "rust-audit",
        "the advisory database is a network round-trip; ci runs it",
    ),
];

/// Whether the project under `root` is a Rust project at all.
pub fn is_rust_project(root: &Path) -> bool {
    root.join("Cargo.toml").is_file()
}

/// A gate that did not run, reported as passed and saying why.
fn skipped(cfg: &Config, name: &str, why: &str) -> GateResult {
    let mut report = Report::new(name);
    report.summary = format!("skipped — {why}");
    report.extra = Some(serde_json::json!({
        "mode": cfg.mode.as_str(),
        "skipped": true,
        "reason": why,
    }));
    GateResult::finish(report)
}

/// Runs a cargo subcommand as a gate.
fn tool_gate(
    cfg: &Config,
    name: &str,
    subcommand: &str,
    args: &[String],
    rule: &str,
) -> Result<GateResult, String> {
    if !is_rust_project(cfg.root()) {
        return Ok(skipped(cfg, name, "no Cargo.toml"));
    }
    let driver = toolchain::detect_driver_for(cfg.root(), cfg.mode);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = toolchain::run_or_missing(cfg.root(), driver, subcommand, &refs, cfg.verbose)?;
    let mut report = Report::new(name);
    report.extra = Some(serde_json::json!({
        "mode": cfg.mode.as_str(),
        "driver": driver.program(),
        "command": format!("{} {} {}", driver.program(), subcommand, args.join(" ")),
    }));
    if result.success {
        report.summary = format!("{name}: {} succeeded", driver.program());
        return Ok(GateResult::finish(report));
    }
    report.add(Diagnostic::error(
        "",
        rule,
        format!(
            "{} {} failed{}",
            driver.program(),
            subcommand,
            exit_suffix(result.code)
        ),
    ));
    report.summary = format!("{name}: {} failed", driver.program());
    Ok(GateResult::failed(name, report.summary.clone(), report))
}

fn exit_suffix(code: Option<i32>) -> String {
    match code {
        Some(c) => format!(" (exit code {c})"),
        None => String::new(),
    }
}

/// Runs one compiling gate in whichever scope this environment asks for.
fn compiling_gate(
    cfg: &Config,
    name: &str,
    subcommand: &str,
    leaf: &[&str],
    rule: &str,
) -> Result<GateResult, String> {
    let driver = toolchain::detect_driver_for(cfg.root(), cfg.mode);
    let args = match scoped(cfg, driver, leaf)? {
        Some(args) => args,
        None => {
            let why = format!("no crate changed since {}", cfg.base_branch);
            return Ok(skipped(cfg, name, &why));
        }
    };
    tool_gate(cfg, name, subcommand, &args, rule)
}

/// The `rust-fmt` gate.
///
/// `--all` already means every crate in the workspace, and neither cargo nor
/// blaze takes `--workspace` for `fmt`, so this gate never adds it.
pub fn fmt(cfg: &Config) -> Result<GateResult, String> {
    let args = args_of(&["--check", "--all"]);
    tool_gate(cfg, "rust-fmt", "fmt", &args, "rust/fmt")
}

/// The `rust-clippy` gate.
pub fn clippy(cfg: &Config) -> Result<GateResult, String> {
    compiling_gate(
        cfg,
        "rust-clippy",
        "clippy",
        &["--all-targets", "--", "-D", "warnings"],
        "rust/clippy",
    )
}

/// The `rust-test` gate.
pub fn test(cfg: &Config) -> Result<GateResult, String> {
    compiling_gate(cfg, "rust-test", "test", &["--all-targets"], "rust/test")
}

/// The `rust-build` gate.
pub fn build(cfg: &Config) -> Result<GateResult, String> {
    compiling_gate(cfg, "rust-build", "build", &["--all-targets"], "rust/build")
}

/// The `rust-doc` gate.
pub fn doc(cfg: &Config) -> Result<GateResult, String> {
    let args = whole_workspace(cfg, &["--no-deps"]);
    tool_gate(cfg, "rust-doc", "doc", &args, "rust/doc")
}

/// One gate: the name it reports under, and the function that runs it.
type GateRunner = (&'static str, fn(&Config) -> Result<GateResult, String>);

/// Every Rust gate, in the order `rust` runs them.
const GATE_RUNNERS: [GateRunner; 10] = [
    ("rust-fmt", fmt),
    ("rust-manifest", manifest_gate),
    ("rust-unsafe", unsafe_code),
    ("rust-panic", panic_free),
    ("rust-msrv", msrv),
    ("rust-clippy", clippy),
    ("rust-build", build),
    ("rust-test", test),
    ("rust-doc", doc),
    ("rust-audit", audit),
];

/// Runs every Rust gate and returns their combined verdict.
///
/// A `dev` run reports the slow gates as skipped instead of silently
/// omitting them, so the output never looks like a full pass.
pub fn run_all(cfg: &Config) -> Result<Vec<(String, GateResult)>, String> {
    let mut results = Vec::with_capacity(GATE_RUNNERS.len());
    for (name, run) in GATE_RUNNERS {
        match skipped_in_dev(name) {
            Some(why) if cfg.is_dev() => {
                results.push((name.to_string(), skipped(cfg, name, why)));
            }
            _ => results.push((name.to_string(), run(cfg)?)),
        }
    }
    Ok(results)
}

/// Why `dev` leaves `name` out, when it does.
fn skipped_in_dev(name: &str) -> Option<&'static str> {
    SLOW_GATES
        .iter()
        .find(|(slow, _)| *slow == name)
        .map(|(_, why)| *why)
}

/// Turns findings into diagnostics.
pub fn findings_to_diagnostics(findings: &[RustFinding]) -> Vec<Diagnostic> {
    findings
        .iter()
        .map(|f| Diagnostic::error(f.path.clone(), f.rule.id(), f.rule.message()).at_line(f.line))
        .collect()
}

#[cfg(test)]
mod tests;
