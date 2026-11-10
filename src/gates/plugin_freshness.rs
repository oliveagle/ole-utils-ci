// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `plugin-freshness` gate: a plugin `.so` must be newer than its
//! sources.
//!
//! A `.so` that is older than the code it was built from is stale: the
//! binary in the repository no longer matches the source. Plugins that have
//! never been built are skipped, because there is nothing to compare.

use std::path::Path;
use std::time::SystemTime;

use crate::config::Config;
use crate::fsutil;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};

/// One stale plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StalePlugin {
    /// Path of the stale `.so`, relative to the project root.
    pub path: String,
    /// The source file that is newer than the plugin.
    pub source: String,
}

/// Scans the plugin directory for stale artifacts.
pub fn scan(root: &Path, source_dir: &str, plugins_dir: &str) -> Vec<StalePlugin> {
    let mut stale = Vec::new();
    let plugins = root.join(plugins_dir);
    if !plugins.is_dir() {
        return stale;
    }
    let sources = collect_source_times(&root.join(source_dir));
    for entry in fsutil::walk_files(&plugins) {
        if !entry.rel.ends_with(".so") {
            continue;
        }
        let Ok(built_at) = std::fs::metadata(&entry.path).and_then(|m| m.modified()) else {
            continue;
        };
        for (source, modified) in &sources {
            if *modified > built_at {
                stale.push(StalePlugin {
                    path: entry.rel.clone(),
                    source: source.clone(),
                });
            }
        }
    }
    stale
}

fn collect_source_times(dir: &Path) -> Vec<(String, SystemTime)> {
    let mut out = Vec::new();
    for entry in fsutil::walk_files(dir) {
        let ext = entry.extension();
        if ext != ".go" && ext != ".rs" && ext != ".c" && ext != ".h" {
            continue;
        }
        if let Ok(modified) = std::fs::metadata(&entry.path).and_then(|m| m.modified()) {
            out.push((entry.rel.clone(), modified));
        }
    }
    out
}

/// Runs the gate and returns its result.
pub fn run(cfg: &Config) -> Result<GateResult, String> {
    let stale = scan(cfg.root(), "src/golang", "plugins");
    let mut report = Report::new("plugin-freshness");
    for s in &stale {
        report.add(Diagnostic::error(
            s.path.clone(),
            "plugin-freshness/stale",
            format!("plugin is older than its source ({})", s.source),
        ));
    }
    if report.ok {
        report.summary = "plugin-freshness: all plugins are up to date".to_string();
        Ok(GateResult::finish(report))
    } else {
        report.summary = format!("plugin-freshness: {} stale plugin(s)", stale.len());
        Ok(GateResult::failed(
            "plugin-freshness",
            report.summary.clone(),
            report,
        ))
    }
}

/// The human-readable body printed when `--output` is not set.
pub fn text_body(result: &GateResult) -> String {
    let mut body = String::from("🔌 Checking plugin freshness...\n");
    for d in &result.report.diagnostics {
        body.push_str(&format!("  ✗ {}: {}\n", d.path, d.message));
    }
    if result.passed() {
        body.push_str("✅ All plugins are up to date\n");
    } else {
        body.push_str(&format!(
            "✗ {} stale plugin(s) found\n",
            result.report.diagnostics.len()
        ));
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_plugins_means_no_findings() {
        let root = std::path::Path::new("/nonexistent-root-for-tests");
        assert!(scan(root, "src/golang", "plugins").is_empty());
    }
}
