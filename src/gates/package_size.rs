// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `package-size` gate: warning-only growth budgets per source package.
//!
//! The Go original measures Go packages only. This rewrite measures Rust
//! packages the same way, because a Rust repository deserves the same
//! feedback loop. The gate never fails by itself; release-artifact budgets
//! belong to [`crate::gates::artifact_size`].

use std::collections::{BTreeMap, BTreeSet};

use crate::config::{limits, Config};
use crate::fsutil;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report, Severity};
use crate::quality_config::QualityConfig;

/// Counting the exported top-level symbols of a source file.
pub mod symbols;

/// Collecting the imports of a source file.
pub mod imports;

pub use imports::collect_imports;
pub use symbols::count_exported_symbols;
/// The warning thresholds that apply to a run.
#[derive(Debug, Clone, Copy)]
pub struct PackageSizeThresholds {
    /// File-count warning threshold.
    pub files: usize,
    /// Non-empty-line warning threshold.
    pub lines: usize,
    /// Exported-symbol warning threshold.
    pub exported: usize,
    /// Import fan-out warning threshold.
    pub imports: usize,
}

/// The measured size of one source package.
#[derive(Debug, Clone, Default)]
pub struct PackageSizeMetric {
    /// The package directory, relative to the project root.
    pub path: String,
    /// Number of source files.
    pub files: usize,
    /// Number of non-empty lines.
    pub lines: usize,
    /// Number of exported symbols.
    pub exported: usize,
    /// The package's imports.
    pub imports: BTreeSet<String>,
}

/// The built-in thresholds.
pub fn default_thresholds() -> PackageSizeThresholds {
    PackageSizeThresholds {
        files: limits::WARN_PACKAGE_FILES,
        lines: limits::WARN_PACKAGE_LINES,
        exported: limits::WARN_PACKAGE_EXPORTED,
        imports: limits::WARN_PACKAGE_IMPORTS,
    }
}

/// Resolves the thresholds for a run, tightening them if the project asked.
pub fn thresholds_for(qcfg: &QualityConfig) -> PackageSizeThresholds {
    let mut t = default_thresholds();
    let Some(cfg) = &qcfg.package_size else {
        return t;
    };
    tighten(&mut t.files, cfg.warn_files);
    tighten(&mut t.lines, cfg.warn_lines);
    tighten(&mut t.exported, cfg.warn_exported_symbols);
    tighten(&mut t.imports, cfg.warn_imports);
    t
}

fn tighten(dst: &mut usize, value: usize) {
    if value > 0 && value < *dst {
        *dst = value;
    }
}

/// Counts the non-empty lines of a source file.
fn count_non_empty_lines(source: &str) -> usize {
    source.lines().filter(|l| !l.trim().is_empty()).count()
}

pub fn scan(root: &std::path::Path) -> Vec<PackageSizeMetric> {
    let mut metrics: BTreeMap<String, PackageSizeMetric> = BTreeMap::new();
    for entry in fsutil::walk_files(root) {
        let ext = entry.extension();
        if ext != ".go" && ext != ".rs" {
            continue;
        }
        let Some(data) = fsutil::read_file(&entry.path) else {
            continue;
        };
        if fsutil::is_generated_file(&data) {
            continue;
        }
        let source = String::from_utf8_lossy(&data);
        let dir = match entry.rel.rfind('/') {
            Some(idx) => entry.rel[..idx].to_string(),
            None => ".".to_string(),
        };
        let metric = metrics
            .entry(dir.clone())
            .or_insert_with(|| PackageSizeMetric {
                path: dir,
                ..Default::default()
            });
        metric.files += 1;
        metric.lines += count_non_empty_lines(&source);
        metric.exported += count_exported_symbols(&source, &ext);
        for import in collect_imports(&source, &ext) {
            metric.imports.insert(import);
        }
    }
    metrics.into_values().collect()
}

/// Builds the warning diagnostics for the measured packages.
pub fn diagnostics(
    metrics: &[PackageSizeMetric],
    thresholds: PackageSizeThresholds,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for m in metrics {
        push_if_over(
            &mut out,
            m,
            "package-size.files",
            m.files,
            thresholds.files,
            "files",
        );
        push_if_over(
            &mut out,
            m,
            "package-size.source-lines",
            m.lines,
            thresholds.lines,
            "source lines",
        );
        push_if_over(
            &mut out,
            m,
            "package-size.exported-symbols",
            m.exported,
            thresholds.exported,
            "exported symbols",
        );
        push_if_over(
            &mut out,
            m,
            "package-size.imports",
            m.imports.len(),
            thresholds.imports,
            "imports",
        );
    }
    out
}

fn push_if_over(
    out: &mut Vec<Diagnostic>,
    metric: &PackageSizeMetric,
    rule: &str,
    value: usize,
    threshold: usize,
    unit: &str,
) {
    if value <= threshold {
        return;
    }
    out.push(Diagnostic::warning(
        metric.path.clone(),
        rule,
        format!("{} has {value} {unit} (warn {threshold})", metric.path),
    ));
}

/// Runs the gate and returns its result.
pub fn run(cfg: &Config) -> Result<GateResult, String> {
    let qcfg = QualityConfig::load(cfg.root())?.unwrap_or_default();
    validate_config(&qcfg)?;
    let thresholds = thresholds_for(&qcfg);
    let metrics = scan(cfg.root());
    let diags = diagnostics(&metrics, thresholds);
    let mut report = Report::new("package-size");
    // Sorted keys, the way Go's `map[string]any` encodes them.
    report.extra = Some(serde_json::json!({
        "packages_scanned": metrics.len(),
        "warn_exports": thresholds.exported,
        "warn_files": thresholds.files,
        "warn_imports": thresholds.imports,
        "warn_lines": thresholds.lines,
    }));
    for d in diags {
        report.add(d);
    }
    report.summary = format!(
        "package-size: {} package(s), {} warning(s)",
        metrics.len(),
        report.counts.warnings
    );
    Ok(GateResult::finish(report))
}

/// Rejects a config that tries to raise the built-in budgets.
pub fn validate_config(qcfg: &QualityConfig) -> Result<(), String> {
    if let Some(section) = &qcfg.ignore.package_size {
        if !section.paths.is_empty() {
            return Err(
                "package-size: path ignores are not allowed for hard quality budgets; lower the budget instead"
                    .to_string(),
            );
        }
    }
    let Some(cfg) = &qcfg.package_size else {
        return Ok(());
    };
    let defaults = default_thresholds();
    let checks = [
        ("warn-files", cfg.warn_files, defaults.files),
        ("warn-lines", cfg.warn_lines, defaults.lines),
        (
            "warn-exported-symbols",
            cfg.warn_exported_symbols,
            defaults.exported,
        ),
        ("warn-imports", cfg.warn_imports, defaults.imports),
    ];
    for (name, value, max) in checks {
        if value > max {
            return Err(format!(
                "package-size: {name}={value} can only tighten built-in max={max}"
            ));
        }
    }
    Ok(())
}

/// The human-readable body printed when `--output` is not set.
pub fn text_body(result: &GateResult) -> String {
    let extra = result.report.extra.as_ref();
    let number = |name: &str| {
        extra
            .and_then(|payload| payload.get(name))
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    };
    let mut body = format!(
        "📦 Checking package size budgets (warn-only: {} files, {} lines, {} exports, {} imports)...\n",
        number("warn_files"),
        number("warn_lines"),
        number("warn_exports"),
        number("warn_imports")
    );
    for d in &result.report.diagnostics {
        body.push_str(&format!("  ⚠ {}: {}\n", d.path, d.message));
    }
    body.push_str(&format!(
        "✅ Package size check completed: {} package(s), {} warning(s), no hard failure\n",
        number("packages_scanned"),
        result.report.counts.warnings
    ));
    body
}

/// The severity a package-size finding carries: always a warning.
pub fn severity() -> Severity {
    Severity::Warning
}

#[cfg(test)]
mod tests;
