// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The Go-compatible ways of running a command.
//!
//! The Go original has four ways to run a tool: `Run` returns the output,
//! `RunSilent` hides it unless the command failed, `RunInteractive` streams
//! it to the terminal and `RunTimeout` bounds the wall clock. The wording
//! of their failures is part of the contract, so it is reproduced here
//! rather than left to the operating system.

use std::time::Duration;

use super::Runner;

/// The combined output of one `runCmd` call plus the error Go builds for it.
#[derive(Debug)]
pub struct GoRun {
    /// Combined stdout and stderr, whatever the exit status was.
    pub output: String,
    /// The decorated failure, or `None` when the command succeeded.
    pub error: Option<String>,
}

impl Runner {
    /// Go's `Runner.Run` / `RunTimeout`: the output always, and the
    /// `<program> <args> failed: <status>\nOutput:\n<output>` error text
    /// when the command failed.
    pub fn run_go(&self, timeout: Duration, program: &str, args: &[&str]) -> GoRun {
        match self.run_with_timeout(timeout, program, args) {
            Ok(out) if out.success => GoRun {
                output: out.output,
                error: None,
            },
            Ok(out) => GoRun {
                output: out.output.clone(),
                error: Some(go_failure(program, args, out.code, &out.output)),
            },
            Err(error) => GoRun {
                output: String::new(),
                error: Some(format!("{error}\nOutput:\n")),
            },
        }
    }

    /// Go's `Runner.RunSilent`: capture both streams, print nothing, and
    /// report a failure with the tail of what the command said.
    pub fn run_silent(
        &self,
        timeout: Duration,
        program: &str,
        args: &[&str],
    ) -> Result<(), String> {
        match self.run_with_timeout(timeout, program, args) {
            Ok(out) if out.success => Ok(()),
            Ok(out) => Err(format!(
                "{program} {} failed: {}\n{}",
                args.join(" "),
                exit_code(out.code),
                tail_4096(&out.output)
            )),
            Err(error) => Err(format!("{error}\n")),
        }
    }
}

/// Go's `runCmd` failure text for a command that ran to completion.
fn go_failure(program: &str, args: &[&str], code: Option<i32>, output: &str) -> String {
    format!(
        "{program} {} failed: {}\nOutput:\n{}",
        args.join(" "),
        exit_code(code),
        output
    )
}

/// Go's `ExitError.Error()` wording: `exit status 3`, or `signal: killed`.
fn exit_code(code: Option<i32>) -> String {
    match code {
        Some(code) => format!("exit status {code}"),
        None => "signal: killed".to_string(),
    }
}

/// Go's `ExitError.Error()` wording: `exit status 3`, or `signal: killed`.
pub(crate) fn exit_text(status: &std::process::ExitStatus) -> String {
    exit_code(status.code())
}

/// The last 4096 bytes of a failed command's output, prefixed the way the
/// Go original truncates it. The cut lands on a char boundary so a
/// multi-byte sequence at the edge cannot panic.
pub fn tail_4096(output: &str) -> String {
    if output.len() <= 4096 {
        return output.to_string();
    }
    let mut start = output.len() - 4096;
    while start < output.len() && !output.is_char_boundary(start) {
        start += 1;
    }
    format!("... [truncated] ...\n{}", &output[start..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_command_reports_its_output() {
        let runner = Runner::new(".", false);
        let result = runner.run_go(
            Duration::from_secs(30),
            "sh",
            &["-c", "printf boom >&2; exit 3"],
        );
        let error = result.error.expect("failure");
        assert!(error.contains("sh -c printf boom >&2; exit 3 failed: exit status 3"));
        assert!(error.contains("Output:"));
        assert!(error.contains("boom"));
    }

    #[test]
    fn a_successful_command_has_no_error() {
        let runner = Runner::new(".", false);
        let result = runner.run_go(Duration::from_secs(30), "sh", &["-c", "printf hi"]);
        assert!(result.error.is_none());
        assert_eq!(result.output, "hi");
    }

    #[test]
    fn the_tail_keeps_the_last_4096_bytes() {
        let long = "x".repeat(5000);
        let tail = tail_4096(&long);
        assert!(tail.starts_with("... [truncated] ...\n"));
        assert_eq!(tail.len(), "... [truncated] ...\n".len() + 4096);
        assert_eq!(tail_4096("short"), "short");
    }
}
