// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The quality-gate subcommands.

use crate::commands::{parse_global, ExitCode};
use crate::config::Config;
use crate::gates::{self, GateResult};

/// Runs one of the pure-analysis gates and returns the exit code.
pub fn run_gate(name: &str, cfg: &Config) -> Result<ExitCode, String> {
    let result = match name {
        "bugs" => gates::bugs::run(cfg)?,
        "sanity" => gates::sanity::run(cfg)?,
        "license" => gates::license::run(cfg)?,
        "structure" => gates::structure::run(cfg)?,
        "file-length" => gates::file_length::run(cfg)?,
        "complexity" => gates::complexity::run(cfg)?,
        "package-size" => gates::package_size::run(cfg)?,
        "artifact-size" => gates::artifact_size::run(cfg)?,
        "goversion" => gates::goversion::run(cfg)?,
        other => return Err(format!("unknown gate: {other}")),
    };
    Ok(emit_gate(name, cfg, &result))
}

/// Prints the gate result in the format the caller asked for.
pub fn emit_gate(name: &str, cfg: &Config, result: &GateResult) -> ExitCode {
    match name {
        "bugs" => gates::emit_text(
            cfg,
            result,
            &gates::bugs::text_body(result, scan_count(result)),
        ),
        "sanity" => gates::emit_text(cfg, result, &gates::sanity::text_body(result)),
        "license" => gates::emit_text(cfg, result, &gates::license::text_body(result)),
        "structure" => gates::emit_text(cfg, result, &gates::structure::text_body(result)),
        "file-length" => gates::emit_text(cfg, result, &gates::file_length::text_body(result)),
        "complexity" => gates::emit_text(cfg, result, &complexity_body(cfg, result)),
        "package-size" => gates::emit_text(cfg, result, &gates::package_size::text_body(result)),
        "artifact-size" => gates::emit_text(cfg, result, &gates::artifact_size::text_body(result)),
        "goversion" => gates::emit_text(cfg, result, &gates::goversion::text_body(result)),
        _ => gates::emit(cfg, result),
    }
}

fn complexity_body(cfg: &Config, result: &GateResult) -> String {
    let qcfg = crate::quality_config::QualityConfig::load(cfg.root())
        .ok()
        .flatten()
        .unwrap_or_default();
    let thresholds = gates::complexity::effective_thresholds(cfg, &qcfg);
    let total = result
        .report
        .extra
        .as_ref()
        .and_then(|e| e.get("functions_scanned"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    gates::complexity::text_body(result, total, thresholds)
}

fn scan_count(result: &GateResult) -> usize {
    result
        .report
        .extra
        .as_ref()
        .and_then(|e| e.get("files_scanned"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize
}

/// Parses the global flags for a gate command.
pub fn parse(cfg: &mut Config, args: &[String]) -> Result<Vec<String>, String> {
    parse_global(cfg, args)
}
