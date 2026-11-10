// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `rust-manifest` and `rust-msrv` gates.

use crate::config::Config;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};

use super::is_rust_project;
use super::manifest;

/// The `rust-manifest` gate.
pub fn manifest_gate(cfg: &Config) -> Result<GateResult, String> {
    if !is_rust_project(cfg.root()) {
        let mut report = Report::new("rust-manifest");
        report.summary = "rust-manifest: no Cargo.toml, skipped".to_string();
        return Ok(GateResult::finish(report));
    }
    let (facts, findings) = manifest::analyze(cfg.root())?;
    let mut report = Report::new("rust-manifest");
    report.extra = Some(serde_json::json!({
        "name": facts.name,
        "version": facts.version,
        "rust_version": facts.rust_version,
        "license": facts.license,
        "dependencies": facts.dependency_count,
        "dev_dependencies": facts.dev_dependency_count,
    }));
    for f in &findings {
        report.add(Diagnostic::error(
            f.key.clone(),
            f.rule.clone(),
            f.message.clone(),
        ));
    }
    if report.ok {
        report.summary = "rust-manifest: Cargo.toml is clean".to_string();
        Ok(GateResult::finish(report))
    } else {
        report.summary = format!("rust-manifest: {} finding(s)", findings.len());
        Ok(GateResult::failed(
            "rust-manifest",
            report.summary.clone(),
            report,
        ))
    }
}

/// The `rust-msrv` gate: the declared MSRV must not exceed the installed
/// toolchain.
pub fn msrv(cfg: &Config) -> Result<GateResult, String> {
    if !is_rust_project(cfg.root()) {
        let mut report = Report::new("rust-msrv");
        report.summary = "rust-msrv: no Cargo.toml, skipped".to_string();
        return Ok(GateResult::finish(report));
    }
    let (facts, _) = manifest::analyze(cfg.root())?;
    let mut report = Report::new("rust-msrv");
    let Some(installed) = installed_rustc_version() else {
        report.summary = "rust-msrv: rustc not installed, skipped".to_string();
        return Ok(GateResult::finish(report));
    };
    report.extra = Some(serde_json::json!({
        "declared": facts.rust_version,
        "installed": installed,
    }));
    if !facts.rust_version.is_empty() && version_gt(&facts.rust_version, &installed) {
        report.add(Diagnostic::error(
            "Cargo.toml",
            "rust/msrv",
            format!(
                "declared rust-version {} is newer than the installed {installed}",
                facts.rust_version
            ),
        ));
    }
    if report.ok {
        report.summary = format!("rust-msrv: {} is supported", facts.rust_version);
        Ok(GateResult::finish(report))
    } else {
        report.summary = "rust-msrv: declared MSRV is not supported".to_string();
        Ok(GateResult::failed(
            "rust-msrv",
            report.summary.clone(),
            report,
        ))
    }
}

/// The version of the installed `rustc`.
pub fn installed_rustc_version() -> Option<String> {
    let output = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let rest = text.split_whitespace().nth(1)?;
    Some(rest.to_string())
}

/// Whether version `a` is greater than version `b`, comparing numeric parts.
pub fn version_gt(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect::<Vec<u64>>()
    };
    let left = parse(a);
    let right = parse(b);
    for i in 0..left.len().max(right.len()) {
        let l = left.get(i).copied().unwrap_or(0);
        let r = right.get(i).copied().unwrap_or(0);
        if l != r {
            return l > r;
        }
    }
    false
}
