// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `coverage` subcommand: line coverage against the threshold.
//!
//! The Go original runs `go test -coverprofile`, filters the generated code
//! out of the profile, asks `go tool cover` for the per-function numbers,
//! averages them per package and compares the result with the threshold.
//! A package that falls short is named, and the report repeats what the
//! operator is expected to do about it.

mod analyze;

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::Config;
use crate::fsutil;
use crate::runner::{tail_4096, Runner};

use super::covergen;
use super::install;

/// The wall-clock budget the coverage run gets.
const COVERAGE_TIMEOUT: Duration = Duration::from_secs(900);

/// The `coverage` subcommand: line coverage against the threshold.
pub fn coverage(cfg: &Config) -> Result<i32, String> {
    println!(
        "📊 Checking coverage (threshold: {}%)...",
        cfg.coverage_threshold
    );
    let generated = cfg.cover_profile.is_empty();
    let profile = if generated {
        cfg.root().join("coverage.out")
    } else {
        PathBuf::from(&cfg.cover_profile)
    };
    let outcome = measure(cfg, &profile, generated);
    if generated {
        let _ = std::fs::remove_file(&profile);
    }
    outcome
}

/// Runs the coverage measurement and reports it.
fn measure(cfg: &Config, profile: &Path, generated: bool) -> Result<i32, String> {
    if generated {
        install::install_dependencies(cfg)
            .map_err(|e| format!("coverage failed: dependency install error: {e}"))?;
        run_tests(cfg, profile)?;
    }
    let runner = Runner::new(cfg.root(), cfg.verbose);
    if !cfg.include_generated {
        filter_generated(profile, cfg.root(), cfg.verbose)?;
    }
    let func = format!("-func={}", profile.display());
    let result = runner.run_go(COVERAGE_TIMEOUT, "go", &["tool", "cover", &func]);
    if let Some(error) = result.error {
        return Err(format!("failed to parse coverage: {error}"));
    }
    report(cfg, &runner, profile, &result.output)
}

/// Runs `go test -coverprofile` and writes the profile.
fn run_tests(cfg: &Config, profile: &Path) -> Result<(), String> {
    let runner = Runner::new(cfg.root(), cfg.verbose);
    let mut args = vec![
        "test".to_string(),
        format!("-coverprofile={}", profile.display()),
        format!("-covermode={}", cfg.coverage_mode),
    ];
    if !cfg.build_tags.is_empty() {
        args.push(format!("-tags={}", cfg.build_tags));
    }
    args.push("./...".to_string());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    if cfg.verbose {
        return runner
            .run_interactive("go", &refs)
            .map_err(|e| format!("coverage test failed (mode={}): {e}", cfg.coverage_mode));
    }
    let result = runner.run_go(COVERAGE_TIMEOUT, "go", &refs);
    if let Some(error) = result.error {
        return Err(format!(
            "coverage test failed (mode={}): {error}\n{}",
            cfg.coverage_mode,
            tail_4096(&result.output)
        ));
    }
    Ok(())
}

/// Drops the generated code out of the profile, in place.
///
/// Filtering the profile rather than the report keeps every downstream
/// tool — the HTML report, the cobertura converters — on the same file.
fn filter_generated(profile: &Path, root: &Path, verbose: bool) -> Result<(), String> {
    let content = std::fs::read_to_string(profile).map_err(|e| {
        format!(
            "filter generated code out of coverprofile: open {}: {}",
            profile.display(),
            crate::commands::ci_recipe_util::go_errno(&e)
        )
    })?;
    let mut matcher = covergen::Matcher::new(root);
    let (kept, dropped, filtered) = covergen::filter_cover_profile(&content, &mut matcher);
    std::fs::write(profile, filtered).map_err(|e| {
        format!(
            "filter generated code out of coverprofile: open {}: {}",
            profile.display(),
            crate::commands::ci_recipe_util::go_errno(&e)
        )
    })?;
    if dropped > 0 && verbose {
        println!(
            "  · filtered {dropped} generated coverprofile entries (kept {kept}); pass --include-generated to disable"
        );
    }
    Ok(())
}

/// Prints the coverage and writes the reports.
fn report(cfg: &Config, runner: &Runner, profile: &Path, output: &str) -> Result<i32, String> {
    let coverage = analyze::parse(output);
    for (package, stat) in &coverage.packages {
        println!("  {package:<70} {:.1}%", stat.average());
    }
    println!("\n  Total coverage: {:.1}%", coverage.total);
    let failures = analyze::failures(&coverage, cfg.coverage_threshold, &cfg.coverage_allowlist);
    if !failures.is_empty() || coverage.total < cfg.coverage_threshold as f64 {
        return Err(analyze::remediation(
            &failures,
            coverage.total,
            cfg.coverage_threshold,
        ));
    }
    write_html(cfg, runner, profile)?;
    write_json(cfg, coverage.total)?;
    println!("✅ Coverage check passed");
    Ok(0)
}

/// Writes the HTML report.
fn write_html(cfg: &Config, runner: &Runner, profile: &Path) -> Result<(), String> {
    if cfg.coverage_html_path.is_empty() {
        return Ok(());
    }
    let html = fsutil::go_join(cfg.root(), &cfg.coverage_html_path);
    let html_text = html.to_string_lossy().into_owned();
    let func = format!("-html={}", profile.display());
    let result = runner.run_go(
        COVERAGE_TIMEOUT,
        "go",
        &["tool", "cover", &func, "-o", html_text.as_str()],
    );
    if let Some(error) = result.error {
        return Err(format!("failed to generate HTML report: {error}"));
    }
    println!("  ✓ HTML report: {}", cfg.coverage_html_path);
    Ok(())
}

/// Writes the JSON report.
fn write_json(cfg: &Config, total: f64) -> Result<(), String> {
    if cfg.coverage_json_path.is_empty() {
        return Ok(());
    }
    let path = fsutil::go_join(cfg.root(), &cfg.coverage_json_path);
    let data = analyze::json_report(total, cfg.coverage_threshold);
    std::fs::write(&path, data).map_err(|e| {
        format!(
            "failed to write JSON report: open {}: {}",
            path.display(),
            crate::commands::ci_recipe_util::go_errno(&e)
        )
    })?;
    println!("  ✓ JSON report: {}", cfg.coverage_json_path);
    Ok(())
}
