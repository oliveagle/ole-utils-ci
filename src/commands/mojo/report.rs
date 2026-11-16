// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! What a gate does with what the tool said.
//!
//! Three gates out of five answer the same way — a run per input, findings collected, one verdict —
//! so that answer lives here instead of in each of them.

use crate::commands::lang_tool;
use crate::commands::mojo::sources::{self, Project};
use crate::config::Config;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};

/// Records a failed tool run as findings, or as the tool's own sentence when it named no file.
pub(super) fn absorb(report: &mut Report, output: &str, rule: &str, subject: &str) {
    let findings = lang_tool::parse_findings(output);
    if findings.is_empty() {
        report.add(Diagnostic::error(
            subject,
            rule,
            lang_tool::blame_of(output),
        ));
    } else {
        for finding in findings {
            report.add(finding);
        }
    }
}

/// Turns a report into the gate's answer, with a summary that says what was checked.
pub(super) fn emit_report(cfg: &Config, report: Report, gate: &str, ok_summary: &str) -> i32 {
    crate::gates::emit(cfg, &verdict_of(cfg, report, gate, ok_summary))
}

/// The result a report becomes: a pass with its summary, or a failure carrying every finding.
pub(super) fn verdict_of(
    cfg: &Config,
    mut report: Report,
    gate: &str,
    ok_summary: &str,
) -> GateResult {
    if report.ok {
        report.summary = if cfg.dry_run {
            format!("{gate}: would run ({ok_summary})")
        } else {
            format!("{gate}: {ok_summary}")
        };
        return GateResult::finish(report);
    }
    let found = report.diagnostics.len();
    let summary = format!("{gate}: {found} finding(s)");
    report.summary = summary.clone();
    GateResult::failed(gate, summary, report)
        .failing(format!("{found} finding(s)"), format!("{found} finding(s)"))
}

/// Says a gate had nothing of its kind to check, and passes.
///
/// A project with no Mojo sources is not a project this group has an opinion about; a project
/// whose sources are all `.🔥` is one the caller has to be told about, because the toolchain
/// stopped reading that extension and every gate above would otherwise skip silently.
pub(super) fn skip(cfg: &Config, gate: &str, project: &Project, why: &str) -> Result<i32, String> {
    // Nothing to check is a skip; a project whose only sources are in an extension the toolchain
    // refuses is an error. The difference is whether there was something here that *should* have
    // been checked: a green run over files nobody read is exactly the false pass this group is
    // built to avoid.
    if project.modules.is_empty() && !project.legacy.is_empty() {
        return Err(format!(
            "{gate}: {} source file(s) use `{}`, which the Mojo toolchain stopped accepting — \
             rename them to `{}` before this gate can check anything",
            project.legacy.len(),
            sources::LEGACY_EXTENSION,
            sources::MOJO_EXTENSION
        ));
    }
    if !project.legacy.is_empty() {
        println!(
            "  ⚠ {} file(s) still use `{}`, which the Mojo toolchain stopped accepting; rename them to `{}`",
            project.legacy.len(),
            sources::LEGACY_EXTENSION,
            sources::MOJO_EXTENSION
        );
    }
    println!("  ⚠ Nothing for {gate} to do ({why}); skipping");
    let mut report = Report::new(gate);
    report.summary = format!("{gate}: skipped — {why}");
    Ok(crate::gates::emit(cfg, &GateResult::finish(report)))
}
