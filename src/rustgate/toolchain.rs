// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Running the Rust toolchain.
//!
//! The Rust gates drive `cargo` the same way the Go gates drive `go`: the
//! gate runs the real tool and turns its exit status into a verdict. The
//! build tool itself is [blaze](https://github.com/indexable-inc/blaze),
//! which answers `build`, `check`, `run`, `test` and `clippy`; when it is not
//! installed the gate falls back to `cargo`.

use std::path::Path;
use std::time::Duration;

use crate::runner::Runner;

/// The build driver to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    /// The blaze daemon, which keeps the workspace compiled in memory.
    Blaze,
    /// Plain cargo.
    Cargo,
}

impl Driver {
    /// The program name to invoke.
    pub fn program(self) -> &'static str {
        match self {
            Driver::Blaze => "blaze",
            Driver::Cargo => "cargo",
        }
    }
}

/// Picks the build driver to use for `root`.
///
/// `blaze` wins when it is installed, unless `DEVINE_UTILS_RUST_DRIVER` says
/// otherwise. The environment variable accepts `cargo` or `blaze`.
pub fn detect_driver(root: &Path) -> Driver {
    match std::env::var("DEVINE_UTILS_RUST_DRIVER").as_deref() {
        Ok("cargo") => return Driver::Cargo,
        Ok("blaze") => return Driver::Blaze,
        _ => {}
    }
    if !Runner::command_exists("blaze") {
        return Driver::Cargo;
    }
    if Runner::command_exists("cargo") && has_cargo_toml(root) {
        return Driver::Blaze;
    }
    Driver::Cargo
}

fn has_cargo_toml(root: &Path) -> bool {
    if root.join("Cargo.toml").is_file() {
        return true;
    }
    std::fs::read_to_string(root.join("Cargo.toml"))
        .ok()
        .map(|_| true)
        .unwrap_or(false)
}

/// Whether the driver can run the given subcommand.
pub fn supports(driver: Driver, subcommand: &str) -> bool {
    match driver {
        Driver::Cargo => true,
        Driver::Blaze => matches!(
            subcommand,
            "build" | "check" | "run" | "test" | "clippy" | "fmt" | "doc" | "tree" | "metadata"
        ),
    }
}

/// The result of running a cargo/blaze subcommand.
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Whether the command succeeded.
    pub success: bool,
    /// The combined stdout and stderr.
    pub output: String,
    /// The exit code, when the process ran to completion.
    pub code: Option<i32>,
}

/// Runs a cargo (or blaze) subcommand in `root`.
pub fn run(
    root: &Path,
    driver: Driver,
    subcommand: &str,
    args: &[&str],
    verbose: bool,
) -> Result<ToolResult, String> {
    if !supports(driver, subcommand) {
        return Err(format!(
            "{} has no {subcommand} subcommand; use cargo instead",
            driver.program()
        ));
    }
    let runner = Runner::new(root, verbose);
    let mut all: Vec<&str> = vec![subcommand];
    all.extend_from_slice(args);
    let out = runner.run_with_timeout(Duration::from_secs(1800), driver.program(), &all)?;
    Ok(ToolResult {
        success: out.success,
        output: out.output,
        code: out.code,
    })
}

/// Runs a cargo subcommand and reports a friendly error when the toolchain is
/// missing.
pub fn run_or_missing(
    root: &Path,
    driver: Driver,
    subcommand: &str,
    args: &[&str],
    verbose: bool,
) -> Result<ToolResult, String> {
    run(root, driver, subcommand, args, verbose).map_err(|e| {
        if e.contains("No such file or directory") || e.contains("not found") {
            format!(
                "{driver_name} is not installed: {e}",
                driver_name = driver.program()
            )
        } else {
            e
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_can_be_forced_by_the_environment() {
        // The default resolution must never pick a driver that is missing.
        let driver = detect_driver(Path::new("/nonexistent-root-for-tests"));
        assert!(matches!(driver, Driver::Cargo | Driver::Blaze));
    }

    #[test]
    fn blaze_supports_only_its_own_subcommands() {
        assert!(supports(Driver::Blaze, "build"));
        assert!(supports(Driver::Blaze, "clippy"));
        assert!(!supports(Driver::Blaze, "install"));
        assert!(supports(Driver::Cargo, "install"));
    }
}
