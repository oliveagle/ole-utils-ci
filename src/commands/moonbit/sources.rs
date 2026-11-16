// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The per-source stage of the MoonBit gate: size and complexity.

use crate::fsutil;
use crate::gates::complexity;
use crate::output::{Diagnostic, Report};

use super::flags::MoonbitFlags;
use super::report::Totals;

pub(super) fn check_source(
    flags: &MoonbitFlags,
    source: &fsutil::Entry,
    report: &mut Report,
    totals: &mut Totals,
    files: &mut Vec<serde_json::Value>,
) {
    let mut payload = serde_json::json!({ "file": source.rel });
    let size = std::fs::metadata(&source.path)
        .map(|meta| meta.len())
        .unwrap_or(0);
    if size > flags.max_file_size as u64 {
        report.add(
            Diagnostic::error(
                &source.rel,
                "moonbit/file-size",
                format!(
                    "source is {} bytes, over --max-file-size {}",
                    size, flags.max_file_size
                ),
            )
            .at_line(1),
        );
        files.push(payload);
        return;
    }
    let data = std::fs::read(&source.path).unwrap_or_default();
    let text = String::from_utf8_lossy(&data);
    let metrics = complexity::lang::scan_file(&source.rel, &text, "moonbit");
    for metric in &metrics {
        if metric.complexity > flags.complexity_warn {
            report.add(
                Diagnostic::error(
                    &source.rel,
                    "moonbit/complexity",
                    format!(
                        "{} has complexity {} over budget {}",
                        metric.function, metric.complexity, flags.complexity_warn
                    ),
                )
                .at_line(metric.line),
            );
            totals.complexity_errors += 1;
        }
    }
    payload["complexity"] = serde_json::json!(metrics
        .iter()
        .map(|metric| serde_json::json!({
            "complexity": metric.complexity,
            "function": metric.function,
            "line": metric.line,
        }))
        .collect::<Vec<_>>());
    files.push(payload);
}
