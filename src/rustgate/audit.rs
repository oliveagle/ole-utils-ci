// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `rust-audit` gate: dependency advisories and licence audit.
//!
//! The gate uses whichever audit tool is installed. `cargo-deny` is
//! preferred because it also checks the licences; `cargo-audit` covers the
//! advisory database. When neither is installed the gate reports that nothing
//! was checked instead of pretending the dependency tree is clean.

use std::time::Duration;

use crate::config::Config;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};
use crate::runner::Runner;

use super::exit_suffix;
use super::is_rust_project;

/// The `rust-audit` gate: dependency advisories and licence audit.
pub fn audit(cfg: &Config) -> Result<GateResult, String> {
    if !is_rust_project(cfg.root()) {
        let mut report = Report::new("rust-audit");
        report.summary = "rust-audit: no Cargo.toml, skipped".to_string();
        return Ok(GateResult::finish(report));
    }
    let mut report = Report::new("rust-audit");
    let mut tools = Vec::new();
    if Runner::command_exists("cargo-deny") {
        tools.push("cargo-deny");
        let result = Runner::new(cfg.root(), cfg.verbose)
            .run_with_timeout(
                Duration::from_secs(600),
                "cargo-deny",
                &["check", "advisories", "licenses"],
            )
            .map_err(|e| e.to_string())?;
        if !result.success {
            report.add(Diagnostic::error(
                "",
                "rust/audit",
                format!("cargo-deny check failed{}", exit_suffix(result.code)),
            ));
            report.summary = "rust-audit: cargo-deny reported problems".to_string();
            return Ok(GateResult::failed(
                "rust-audit",
                report.summary.clone(),
                report,
            ));
        }
    }
    if Runner::command_exists("cargo-audit") {
        tools.push("cargo-audit");
        let result = Runner::new(cfg.root(), cfg.verbose)
            .run_with_timeout(Duration::from_secs(600), "cargo-audit", &["--json"])
            .map_err(|e| e.to_string())?;
        if !result.success {
            report.add(Diagnostic::error(
                "",
                "rust/audit",
                format!(
                    "cargo-audit reported advisories{}",
                    exit_suffix(result.code)
                ),
            ));
        }
    }
    report.extra = Some(serde_json::json!({ "tools": tools }));
    if report.ok {
        report.summary = if tools.is_empty() {
            "rust-audit: no audit tool installed, nothing checked".to_string()
        } else {
            format!("rust-audit: {} reported no advisories", tools.join(", "))
        };
        Ok(GateResult::finish(report))
    } else {
        report.summary = "rust-audit: advisories found".to_string();
        Ok(GateResult::failed(
            "rust-audit",
            report.summary.clone(),
            report,
        ))
    }
}
