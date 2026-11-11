// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `test` subcommand: run the unit tests with the race detector.
//!
//! The Go original installs the dependencies first, then runs `go test`
//! with the race detector when cgo is available. The verbose run streams the
//! output; the quiet run keeps it back and shows the tail of it only when
//! the tests fail.

use std::time::Duration;

use crate::config::Config;
use crate::runner::{tail_4096, Runner};

use super::install;

/// The wall-clock budget the test run gets.
const TEST_TIMEOUT: Duration = Duration::from_secs(300);

/// The `test` subcommand: run the unit tests with the race detector.
pub fn test(cfg: &Config) -> Result<i32, String> {
    println!("🧪 Running tests...");
    install::install_dependencies(cfg)
        .map_err(|e| format!("tests failed: dependency install error: {e}"))?;
    let runner = Runner::new(cfg.root(), cfg.verbose);
    let use_race = race_supported();
    let args = test_args(cfg, use_race);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    println!("{}", race_notice(use_race));
    if cfg.verbose {
        runner
            .run_interactive("go", &refs)
            .map_err(|e| format!("tests failed: {e}"))?;
    } else {
        let result = runner.run_go(TEST_TIMEOUT, "go", &refs);
        if let Some(error) = result.error {
            return Err(format!(
                "tests failed: {error}\n{}",
                tail_4096(&result.output)
            ));
        }
    }
    println!("✅ All tests passed");
    Ok(0)
}

/// The `go test` command line.
///
/// The verbose flag comes last, the way the Go original appends it.
fn test_args(cfg: &Config, use_race: bool) -> Vec<String> {
    let mut args = vec!["test".to_string()];
    if use_race {
        args.push("-race".to_string());
    }
    if !cfg.build_tags.is_empty() {
        args.push(format!("-tags={}", cfg.build_tags));
    }
    if cfg.test_parallel > 0 {
        args.push("-parallel".to_string());
        args.push(cfg.test_parallel.to_string());
    }
    args.push("./...".to_string());
    if cfg.verbose {
        args.push("-v".to_string());
    }
    args
}

/// Whether the race detector can be used.
///
/// `-race` needs cgo; when `CGO_ENABLED=0` the Go toolchain refuses it, so
/// the flag is dropped instead of failing the run. An unset variable counts
/// as disabled on Windows, where the toolchain ships without a C compiler.
pub fn race_supported() -> bool {
    match std::env::var("CGO_ENABLED").as_deref() {
        Ok("0") => false,
        Ok("1") => true,
        _ => !cfg!(target_os = "windows"),
    }
}

/// The line that tells the operator what the race detector did.
pub fn race_notice(use_race: bool) -> &'static str {
    if use_race {
        "  -race enabled"
    } else {
        "  -race skipped (CGO_ENABLED=0 or a Windows Go toolchain without cgo)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_verbose_flag_comes_last() {
        let cfg = Config {
            verbose: true,
            test_parallel: 4,
            build_tags: "tag".to_string(),
            ..Config::default()
        };
        let args = test_args(&cfg, true);
        assert_eq!(args[0], "test");
        assert_eq!(args[1], "-race");
        assert_eq!(args[2], "-tags=tag");
        assert_eq!(args[3], "-parallel");
        assert_eq!(args[4], "4");
        assert_eq!(args[5], "./...");
        assert_eq!(args[6], "-v");
    }

    #[test]
    fn the_quiet_run_has_no_verbose_flag() {
        let cfg = Config::default();
        let args = test_args(&cfg, false);
        assert_eq!(args, vec!["test", "./..."]);
    }
}
