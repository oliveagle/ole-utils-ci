// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The source-scanning Rust gates: `rust-unsafe` and `rust-panic`.

use crate::config::Config;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};

use super::is_rust_project;
use super::scanner::{self, Rule};

/// The `rust-unsafe` gate.
pub fn unsafe_code(cfg: &Config) -> Result<GateResult, String> {
    source_gate(cfg, "rust-unsafe", &[Rule::Unsafe])
}

/// The `rust-panic` gate: no `unwrap`, `expect` or panic macro in
/// production code.
pub fn panic_free(cfg: &Config) -> Result<GateResult, String> {
    source_gate(
        cfg,
        "rust-panic",
        &[Rule::Unwrap, Rule::Expect, Rule::PanicMacro],
    )
}

/// Runs a source-scanning gate over the production code.
fn source_gate(cfg: &Config, name: &str, rules: &[Rule]) -> Result<GateResult, String> {
    if !is_rust_project(cfg.root()) {
        let mut report = Report::new(name);
        report.summary = format!("{name}: no Cargo.toml, skipped");
        return Ok(GateResult::finish(report));
    }
    let findings = scanner::scan(cfg.root(), rules);
    let mut report = Report::new(name);
    report.extra = Some(serde_json::json!({
        "rules": rules.iter().map(|r| r.id()).collect::<Vec<_>>(),
        "counts": scanner::count_by_rule(&findings),
    }));
    for f in &findings {
        report
            .add(Diagnostic::error(f.path.clone(), f.rule.id(), f.rule.message()).at_line(f.line));
    }
    if report.ok {
        report.summary = format!("{name}: no violations");
        Ok(GateResult::finish(report))
    } else {
        report.summary = format!("{name}: {} violation(s)", findings.len());
        Ok(GateResult::failed(name, report.summary.clone(), report))
    }
}
