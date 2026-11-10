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

pub mod manifest;
pub mod scanner;
pub mod toolchain;

use std::path::Path;

use crate::config::Config;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};
use scanner::RustFinding;

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

/// Whether the project under `root` is a Rust project at all.
pub fn is_rust_project(root: &Path) -> bool {
    root.join("Cargo.toml").is_file()
}

/// Runs a cargo subcommand as a gate.
fn tool_gate(
    cfg: &Config,
    name: &str,
    subcommand: &str,
    args: &[&str],
    rule: &str,
) -> Result<GateResult, String> {
    if !is_rust_project(cfg.root()) {
        let mut report = Report::new(name);
        report.summary = format!("{name}: no Cargo.toml, skipped");
        return Ok(GateResult::finish(report));
    }
    let driver = toolchain::detect_driver(cfg.root());
    let result = toolchain::run_or_missing(cfg.root(), driver, subcommand, args, cfg.verbose)?;
    let mut report = Report::new(name);
    report.extra = Some(serde_json::json!({
        "driver": driver.program(),
        "command": format!("{} {}", driver.program(), subcommand),
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

/// The `rust-fmt` gate.
pub fn fmt(cfg: &Config) -> Result<GateResult, String> {
    tool_gate(cfg, "rust-fmt", "fmt", &["--check", "--all"], "rust/fmt")
}

/// The `rust-clippy` gate.
pub fn clippy(cfg: &Config) -> Result<GateResult, String> {
    tool_gate(
        cfg,
        "rust-clippy",
        "clippy",
        &["--all-targets", "--", "-D", "warnings"],
        "rust/clippy",
    )
}

/// The `rust-test` gate.
pub fn test(cfg: &Config) -> Result<GateResult, String> {
    tool_gate(cfg, "rust-test", "test", &["--all-targets"], "rust/test")
}

/// The `rust-build` gate.
pub fn build(cfg: &Config) -> Result<GateResult, String> {
    tool_gate(cfg, "rust-build", "build", &["--all-targets"], "rust/build")
}

/// The `rust-doc` gate.
pub fn doc(cfg: &Config) -> Result<GateResult, String> {
    tool_gate(
        cfg,
        "rust-doc",
        "doc",
        &["--no-deps", "--", "-D", "rustdoc::broken_intra_doc_links"],
        "rust/doc",
    )
}

/// Runs every Rust gate and returns their combined verdict.
pub fn run_all(cfg: &Config) -> Result<Vec<(String, GateResult)>, String> {
    let mut out = Vec::new();
    out.push(("rust-fmt".to_string(), fmt(cfg)?));
    out.push(("rust-manifest".to_string(), manifest_gate(cfg)?));
    out.push(("rust-unsafe".to_string(), unsafe_code(cfg)?));
    out.push(("rust-panic".to_string(), panic_free(cfg)?));
    out.push(("rust-msrv".to_string(), msrv(cfg)?));
    out.push(("rust-clippy".to_string(), clippy(cfg)?));
    out.push(("rust-build".to_string(), build(cfg)?));
    out.push(("rust-test".to_string(), test(cfg)?));
    out.push(("rust-doc".to_string(), doc(cfg)?));
    out.push(("rust-audit".to_string(), audit(cfg)?));
    Ok(out)
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
