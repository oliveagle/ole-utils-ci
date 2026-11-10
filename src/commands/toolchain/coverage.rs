// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `coverage` subcommand and the coverprofile analyser.

use std::path::Path;
use std::time::Duration;

use crate::config::Config;
use crate::output::{Diagnostic, Report};
use crate::runner::Runner;

/// The `coverage` subcommand: line coverage against the threshold.
pub fn coverage(cfg: &Config) -> Result<i32, String> {
    println!("📈 Measuring line coverage...");
    let runner = Runner::new(cfg.root(), cfg.verbose);
    let profile = "coverage.out";
    let args = [
        "test".to_string(),
        format!("-covermode={}", cfg.coverage_mode),
        format!("-coverprofile={profile}"),
        "./...".to_string(),
    ];
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let result = runner.run_with_timeout(Duration::from_secs(3600), "go", &refs)?;
    if !result.success {
        print!("{}", result.output);
        return Err("coverage run failed".to_string());
    }
    let report = analyze_coverage(cfg, Path::new(profile))?;
    print!("{}", report.render_text());
    if report.ok {
        Ok(0)
    } else {
        Err("coverage threshold not met".to_string())
    }
}

/// Reads a coverprofile and checks it against the configured threshold.
pub fn analyze_coverage(cfg: &Config, profile: &Path) -> Result<Report, String> {
    let content = std::fs::read_to_string(profile)
        .map_err(|e| format!("failed to read {}: {e}", profile.display()))?;
    let (covered, total) = tally_coverage(&content);
    let percent = if total == 0 {
        0.0
    } else {
        (covered as f64 / total as f64) * 100.0
    };
    let mut report = Report::new("coverage");
    report.extra = Some(serde_json::json!({
        "covered": covered,
        "total": total,
        "percent": (percent * 100.0).round() / 100.0,
        "threshold": cfg.coverage_threshold,
    }));
    let threshold = cfg.coverage_threshold;
    if percent + 0.0001 < threshold as f64 {
        report.add(Diagnostic::error(
            "coverage.out",
            "coverage/below-threshold",
            format!("line coverage {percent:.2}% is below the {threshold}% threshold"),
        ));
    }
    if report.ok {
        report.summary = format!("coverage: {percent:.2}% meets the threshold");
    } else {
        report.summary = format!("coverage: {percent:.2}% is below the threshold");
    }
    Ok(report)
}

/// Counts the covered and the total statements of a coverprofile.
///
/// Each line looks like `import/path/file.go:12.34,15.6 3 1`, where the last
/// two numbers are the statement count and the execution count.
fn tally_coverage(content: &str) -> (usize, usize) {
    let mut total = 0usize;
    let mut covered = 0usize;
    for line in content.lines().skip(1) {
        let Some((before, count)) = line.rsplit_once(' ') else {
            continue;
        };
        let statements: usize = before
            .rsplit_once(' ')
            .and_then(|(stmts, _)| stmts.parse().ok())
            .unwrap_or(0);
        total += statements;
        if count.trim() != "0" {
            covered += statements;
        }
    }
    (covered, total)
}
