// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The Rust-specific subcommands.

use crate::commands::parse_global;
use crate::config::Config;
use crate::gates::GateResult;
use crate::rustgate;

/// The name of every Rust subcommand.
pub const COMMANDS: [&str; 11] = [
    "rust",
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

/// Runs one of the Rust subcommands.
pub fn run(name: &str, cfg: &Config) -> Result<i32, String> {
    if name == "rust" {
        return crate::commands::all::run_rust(cfg);
    }
    let result = run_gate(name, cfg)?;
    print!("{}", result.report.render_text());
    if result.passed() {
        Ok(0)
    } else {
        Ok(1)
    }
}

fn run_gate(name: &str, cfg: &Config) -> Result<GateResult, String> {
    match name {
        "rust-fmt" => rustgate::fmt(cfg),
        "rust-clippy" => rustgate::clippy(cfg),
        "rust-test" => rustgate::test(cfg),
        "rust-build" => rustgate::build(cfg),
        "rust-doc" => rustgate::doc(cfg),
        "rust-audit" => rustgate::audit(cfg),
        "rust-manifest" => rustgate::manifest_gate(cfg),
        "rust-unsafe" => rustgate::unsafe_code(cfg),
        "rust-panic" => rustgate::panic_free(cfg),
        "rust-msrv" => rustgate::msrv(cfg),
        other => Err(format!("unknown Rust gate: {other}")),
    }
}

/// Parses the global flags for a Rust subcommand.
pub fn parse(cfg: &mut Config, args: &[String]) -> Result<Vec<String>, String> {
    parse_global(cfg, args)
}
