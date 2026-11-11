// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Running the external tools the gates depend on.
//!
//! The Go original shells out to `go`, `golangci-lint`, `buf` and friends.
//! This module keeps the same shape: a [`Runner`] owns the working directory
//! and the verbosity flag, and every invocation has a wall-clock timeout so a
//! hung toolchain fails the gate instead of hanging CI.

use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Default wall-clock budget for a single tool invocation.
pub const DEFAULT_CMD_TIMEOUT: Duration = Duration::from_secs(300);

/// The result of running an external command.
#[derive(Debug, Clone)]
pub struct CmdOutput {
    /// Combined stdout and stderr.
    pub output: String,
    /// Whether the command exited successfully.
    pub success: bool,
    /// The exit code, when the process ran to completion.
    pub code: Option<i32>,
}

/// Runs external commands in a fixed working directory.
#[derive(Debug, Clone)]
pub struct Runner {
    /// Working directory for every command.
    pub dir: PathBuf,
    /// Print each command before running it.
    pub verbose: bool,
    /// Extra environment variables for every command this runner starts.
    pub env: Vec<(String, String)>,
}

impl Runner {
    /// Creates a runner for `dir`.
    pub fn new(dir: impl Into<PathBuf>, verbose: bool) -> Self {
        Runner {
            dir: dir.into(),
            verbose,
            env: Vec::new(),
        }
    }

    /// Adds one environment variable to every command this runner starts.
    ///
    /// `blaze` reads its sandbox mode when the daemon starts, so the gates
    /// push `BLAZE_SANDBOX=off` through here when the container blocks the
    /// user namespaces the daemon would otherwise isolate itself in.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Runs a command, capturing stdout and stderr together.
    pub fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        Ok(self
            .run_with_timeout(DEFAULT_CMD_TIMEOUT, program, args)?
            .output)
    }

    /// Runs a command with an explicit timeout.
    pub fn run_with_timeout(
        &self,
        timeout: Duration,
        program: &str,
        args: &[&str],
    ) -> Result<CmdOutput, String> {
        if self.verbose {
            println!("[exec] {program} {}", args.join(" "));
        }
        let mut cmd = Command::new(program);
        cmd.args(args)
            .current_dir(&self.dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &self.env {
            cmd.env(key, value);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("{program} {} failed: {e}", args.join(" ")))?;
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let mut output = String::new();
                    if let Some(mut out) = child.stdout.take() {
                        use std::io::Read;
                        let mut buf = String::new();
                        let _ = out.read_to_string(&mut buf);
                        output.push_str(&buf);
                    }
                    if let Some(mut err) = child.stderr.take() {
                        use std::io::Read;
                        let mut buf = String::new();
                        let _ = err.read_to_string(&mut buf);
                        output.push_str(&buf);
                    }
                    let code = status.code();
                    let success = status.success();
                    return Ok(CmdOutput {
                        output,
                        success,
                        code,
                    });
                }
                Ok(None) => {
                    if started.elapsed() > timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(format!(
                            "{program} {} timed out after {}s",
                            args.join(" "),
                            timeout.as_secs()
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => return Err(format!("{program} {} failed: {e}", args.join(" "))),
            }
        }
    }

    /// Runs a command and returns an error containing its output on failure.
    pub fn run_checked(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let out = self.run_with_timeout(DEFAULT_CMD_TIMEOUT, program, args)?;
        if out.success {
            Ok(out.output)
        } else {
            Err(format!(
                "{program} {} failed with exit code {}\nOutput:\n{}",
                args.join(" "),
                out.code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "unknown".into()),
                out.output
            ))
        }
    }

    /// Runs a command and streams its output to the terminal.
    pub fn run_interactive(&self, program: &str, args: &[&str]) -> Result<(), String> {
        if self.verbose {
            println!("[exec] {program} {}", args.join(" "));
        }
        let mut cmd = Command::new(program);
        cmd.args(args).current_dir(&self.dir);
        for (key, value) in &self.env {
            cmd.env(key, value);
        }
        let status = cmd
            .status()
            .map_err(|e| format!("{program} {} failed: {e}", args.join(" ")))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "{program} {} failed with exit code {}",
                args.join(" "),
                status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "unknown".into())
            ))
        }
    }

    /// Whether `program` is available on `PATH`.
    pub fn command_exists(program: &str) -> bool {
        if program.contains('/') {
            return Path::new(program).is_file();
        }
        let path = match std::env::var_os("PATH") {
            Some(p) => p,
            None => return false,
        };
        std::env::split_paths(&path).any(|dir| {
            let candidate = dir.join(program);
            candidate.is_file() && is_executable(&candidate)
        })
    }
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Go's `ExitError.Error()` wording: `exit status 3`, or `signal: killed`.
fn exit_text(status: &std::process::ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("exit status {code}"),
        None => "signal: killed".to_string(),
    }
}

mod pipe;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_missing_commands() {
        assert!(!Runner::command_exists("definitely-not-a-real-command-xyz"));
        assert!(Runner::command_exists("sh"));
    }

    #[test]
    fn captures_output() {
        let r = Runner::new(".", false);
        let out = r.run("sh", &["-c", "echo hi"]).unwrap();
        assert!(out.contains("hi"));
    }

    #[test]
    fn reports_failure_with_output() {
        let r = Runner::new(".", false);
        let err = r
            .run_checked("sh", &["-c", "echo boom >&2; exit 3"])
            .unwrap_err();
        assert!(err.contains("boom"));
        assert!(err.contains("exit code 3"));
    }

    #[test]
    fn passes_extra_environment_to_the_child() {
        let r = Runner::new(".", false).with_env("DEVINE_UTILS_CI_PROBE", "yes");
        let out = r
            .run("sh", &["-c", "printf '%s' \"$DEVINE_UTILS_CI_PROBE\""])
            .unwrap();
        assert_eq!(out, "yes");
    }

    #[test]
    fn pipes_producer_output_into_the_consumer() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        let r = Runner::new(".", false);
        r.run_pipe(&["sh", "-c", "printf abc; printf def >&2"], &["cat"], &out)
            .unwrap();
        let got = std::fs::read_to_string(&out).unwrap();
        let mut chars: Vec<char> = got.chars().collect();
        chars.sort_unstable();
        assert_eq!(chars, vec!['a', 'b', 'c', 'd', 'e', 'f'], "{got}");
    }

    #[test]
    fn reports_consumer_failures() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        let r = Runner::new(".", false);
        let err = r
            .run_pipe(
                &["sh", "-c", "printf abc"],
                &["sh", "-c", "cat >/dev/null; exit 4"],
                &out,
            )
            .unwrap_err();
        assert!(err.contains("exit status 4"), "{err}");
    }
}
