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
//!
//! The two drivers do not accept the same flags, so [`adapt_args`] rewrites
//! cargo's command line for blaze before the process starts.

use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use crate::runner::Runner;

/// Wall-clock budget for one cargo/blaze invocation.
const TOOL_TIMEOUT: Duration = Duration::from_secs(1800);

/// Budget for stopping a daemon that has to be restarted.
const STOP_TIMEOUT: Duration = Duration::from_secs(60);

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
    root.join("Cargo.toml").is_file()
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

/// Rewrites cargo's flags into the set the driver accepts.
///
/// cargo builds every target and lets rustdoc lint the documentation. blaze
/// builds the default programs only, and writes documentation from its own
/// reading of the source with no rustdoc to receive flags, so it takes
/// neither `--all-targets` nor `--no-deps`. It does accept clippy's flags,
/// so those stay as they are.
pub fn adapt_args<'a>(driver: Driver, subcommand: &'a str, args: &'a [&'a str]) -> Vec<&'a str> {
    match driver {
        Driver::Cargo => args.to_vec(),
        Driver::Blaze => match subcommand {
            "build" | "test" => args
                .iter()
                .copied()
                .filter(|arg| *arg != "--all-targets")
                .collect(),
            "doc" => Vec::new(),
            _ => args.to_vec(),
        },
    }
}

/// The complete command line for one invocation.
fn command_line<'a>(driver: Driver, subcommand: &'a str, args: &'a [&'a str]) -> Vec<&'a str> {
    let adapted = adapt_args(driver, subcommand, args);
    let mut all = Vec::with_capacity(adapted.len() + 1);
    all.push(subcommand);
    all.extend(adapted);
    all
}

/// Whether blaze should run without its namespace sandbox.
///
/// An explicit `BLAZE_SANDBOX` always wins. Otherwise the sandbox is turned
/// off when this container refuses the user namespaces blaze would isolate
/// each build in, because a daemon started with a sandbox it cannot create
/// fails every later build too.
pub fn sandbox_off() -> bool {
    match std::env::var("BLAZE_SANDBOX").as_deref() {
        Ok(value) => value == "off",
        Err(_) => !user_namespace_works(),
    }
}

/// Whether this machine lets a process map itself into a user namespace.
///
/// The probe runs once: it spawns `unshare`, which reports the same refusal
/// the blaze daemon would meet.
fn user_namespace_works() -> bool {
    static WORKS: OnceLock<bool> = OnceLock::new();
    *WORKS.get_or_init(|| {
        std::process::Command::new("unshare")
            .args(["--user", "--map-root-user", "true"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            // Without `unshare` there is nothing to probe with, so give the
            // daemon the benefit of the doubt and let its own error decide.
            .unwrap_or(true)
    })
}

/// Whether the output is blaze complaining about its sandbox, not the build.
fn sandbox_failure(output: &str) -> bool {
    output.contains("setgroups") || output.contains("uid_map")
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
    let mut runner = Runner::new(root, verbose);
    let mut restart_daemon = false;
    if driver == Driver::Blaze && sandbox_off() {
        runner = runner.with_env("BLAZE_SANDBOX", "off");
        restart_daemon = true;
    }
    if driver == Driver::Cargo && subcommand == "doc" {
        // `cargo doc` has no `-- -D` passthrough, so rustdoc reads its lint
        // flags from the environment instead.
        runner = runner.with_env("RUSTDOCFLAGS", "-D rustdoc::broken_intra_doc_links");
    }
    let all = command_line(driver, subcommand, args);
    let out = runner.run_with_timeout(TOOL_TIMEOUT, driver.program(), &all)?;
    if restart_daemon && !out.success && sandbox_failure(&out.output) {
        // A daemon keeps the sandbox it started with, so the only way the
        // new setting takes effect is to stop it and let the next build
        // start one that honours `BLAZE_SANDBOX=off`.
        let _ = runner.run_with_timeout(STOP_TIMEOUT, "blaze", &["stop"]);
        let retried = runner.run_with_timeout(TOOL_TIMEOUT, driver.program(), &all)?;
        return Ok(as_tool_result(retried));
    }
    Ok(as_tool_result(out))
}

fn as_tool_result(out: crate::runner::CmdOutput) -> ToolResult {
    ToolResult {
        success: out.success,
        output: out.output,
        code: out.code,
    }
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
mod tests;
