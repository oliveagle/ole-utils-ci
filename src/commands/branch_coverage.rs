// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `branch-coverage` — the gobco branch-coverage gate.
//!
//! The Go original embeds [`gobco`](https://github.com/rillig/gobco), so it
//! never needs a separately installed binary. This rewrite shells out to
//! `gobco` when it is on `PATH`; without it the gate reports that it skipped
//! rather than inventing a coverage number.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::commands::ExitCode;
use crate::config::Config;
use crate::runner::Runner;
use crate::static_regex;

/// gobco's machine-readable summary line, which it prints as its last line.
const SUMMARY: &str = r"(?m)^Branch coverage: ([0-9]+)/([0-9]+) \([0-9]+(?:\.[0-9]+)?%\)$";

/// Runs the branch-coverage gate and returns the exit code.
pub fn branch_coverage(cfg: &Config) -> Result<ExitCode, String> {
    println!("🌿 Checking branch coverage...");
    let Some(captured) = run_gobco(cfg)? else {
        println!("⏭ gobco is not installed; branch coverage skipped");
        return Ok(0);
    };
    let (covered, total) = parse_summary(&captured)?;
    if total == 0 {
        println!("  ⚠ No branches found; threshold check skipped");
        return Ok(0);
    }
    let percentage = covered as f64 / total as f64 * 100.0;
    let threshold = cfg.branch_coverage_threshold;
    if percentage < threshold as f64 {
        return Err(format!(
            "branch coverage {percentage:.1}% ({covered}/{total}) < threshold {threshold}%"
        ));
    }
    println!(
        "✅ Branch coverage check passed: {percentage:.1}% ({covered}/{total}), threshold {threshold}%"
    );
    Ok(0)
}

/// Runs gobco and returns its stdout; `None` when gobco is not installed.
fn run_gobco(cfg: &Config) -> Result<Option<String>, String> {
    if !Runner::command_exists("gobco") {
        if cfg.root().join("go.mod").exists() {
            return Ok(None);
        }
        // gobco is embedded in the Go build, and without a Go module it
        // cannot resolve `./...`; it fails with exactly this message.
        eprintln!("error: pattern \"./...\" must be inside a Go module");
        flush_stdout();
        return Err("branch coverage: gobco failed with exit code 1".to_string());
    }
    let args = gobco_args(cfg);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = Command::new("gobco")
        .args(&refs)
        .current_dir(cfg.root())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("branch coverage: gobco failed: {e}"))?;
    let captured = String::from_utf8_lossy(&output.stdout).to_string();
    print!("{captured}");
    flush_stdout();
    let code = output.status.code().unwrap_or(1);
    if code != 0 {
        return Err(format!(
            "branch coverage: gobco failed with exit code {code}"
        ));
    }
    Ok(Some(captured))
}

/// The argument vector gobco is invoked with.
fn gobco_args(cfg: &Config) -> Vec<String> {
    let mut args = vec!["--branch".to_string()];
    if cfg.verbose {
        args.push("-v".to_string());
    }
    args.push("./...".to_string());
    if !cfg.build_tags.is_empty() {
        args.push("--".to_string());
        args.push(format!("-tags={}", cfg.build_tags));
    }
    args
}

/// Pulls `(covered, total)` out of gobco's summary line.
fn parse_summary(output: &str) -> Result<(i64, i64), String> {
    let re = static_regex::compile(SUMMARY);
    let Some(caps) = re.captures(output) else {
        return Err("branch coverage: gobco summary not found in output".to_string());
    };
    let covered = parse_count(&caps[1])
        .map_err(|e| format!("branch coverage: parse covered branches: {e}"))?;
    let total =
        parse_count(&caps[2]).map_err(|e| format!("branch coverage: parse total branches: {e}"))?;
    if covered < 0 || total < 0 || covered > total {
        return Err(format!(
            "branch coverage: invalid gobco summary: covered={covered} total={total}"
        ));
    }
    Ok((covered, total))
}

/// `strconv.Atoi`'s wording, which the Go original wraps into its error.
fn parse_count(raw: &str) -> Result<i64, String> {
    raw.parse::<i64>()
        .map_err(|_| format!("strconv.Atoi: parsing {raw:?}: value out of range"))
}

/// Makes sure already-written stdout reaches the terminal before an error.
fn flush_stdout() {
    let _ = std::io::stdout().flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_summary_gobco_prints() {
        assert_eq!(
            parse_summary("Branch coverage: 12/14 (85.7%)").unwrap(),
            (12, 14)
        );
        assert_eq!(parse_summary("Branch coverage: 0/0 (0%)").unwrap(), (0, 0));
    }

    #[test]
    fn rejects_a_missing_or_inverted_summary() {
        assert!(parse_summary("nothing here").is_err());
        assert!(parse_summary("Branch coverage: 14/12 (116%)").is_err());
    }

    #[test]
    fn passes_build_tags_through_after_the_separator() {
        let mut cfg = Config::new();
        cfg.build_tags = "test_no_credis".to_string();
        assert_eq!(
            gobco_args(&cfg),
            ["--branch", "./...", "--", "-tags=test_no_credis"]
        );
    }
}
