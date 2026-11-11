// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `devine-utils-ci` — the unified CI quality-gate tool.

use std::io::Write;
use std::process::ExitCode;

mod dispatch;
mod usage;

/// The commands that need nothing but the global flags.
const GATE_COMMANDS: [&str; 12] = [
    "bugs",
    "sanity",
    "license",
    "structure",
    "file-length",
    "complexity",
    "package-size",
    "artifact-size",
    "go-pin",
    "goversion",
    "no-shell-exec",
    "plugin-freshness",
];

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (command, args, timestamped) = split_args(&argv);
    if command.is_empty() {
        print_usage();
        return ExitCode::from(1);
    }
    if timestamped {
        return run_timestamped(&command, &args);
    }
    match dispatch::dispatch(&command, &args) {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(1)
        }
    }
}

/// Splits the command line into the subcommand, its arguments and whether
/// timestamped output was requested.
///
/// Go takes `os.Args[1]` as the command without looking at it, so `--version`
/// and `-h` are commands in their own right and `--ts` is only a flag when it
/// appears *after* the command. An empty argument list is the one case Go
/// special-cases: it prints the usage and exits 1.
fn split_args(argv: &[String]) -> (String, Vec<String>, bool) {
    let timestamped = std::env::var("DEVINE_UTILS_TS").is_ok_and(|v| !v.is_empty());
    let Some(command) = argv.first() else {
        return (String::new(), Vec::new(), timestamped);
    };
    let mut timestamped = timestamped;
    let mut args = Vec::new();
    for arg in &argv[1..] {
        if arg == "--ts" || arg == "--timestamp" {
            timestamped = true;
        } else {
            args.push(arg.clone());
        }
    }
    (command.clone(), args, timestamped)
}

/// Runs a command, prefixing every line of its standard output with a
/// timestamp — the format Go's `tsWriter` produces: `2006-01-02 15:04:05 `
/// in front of each line, including a final line that has no newline.
///
/// The child must not see `DEVINE_UTILS_TS`: this process is already the
/// timestamping wrapper, so letting the variable through would make every
/// child re-exec itself forever. Go only wraps `os.Stdout`, so the child's
/// standard error is passed through untouched.
fn run_timestamped(command: &str, args: &[String]) -> ExitCode {
    let exe =
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("devine-utils-ci"));
    let mut child = std::process::Command::new(exe);
    child.arg(command).args(args);
    child.env_remove("DEVINE_UTILS_TS");
    let Ok(output) = child.output() else {
        eprintln!("Error: failed to run the command");
        return ExitCode::from(1);
    };
    let mut out = std::io::stdout().lock();
    for line in output.stdout.split_inclusive(|&byte| byte == b'\n') {
        let _ = write!(out, "{} ", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
        let _ = out.write_all(line);
    }
    let _ = out.flush();
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(&output.stderr);
    ExitCode::from(output.status.code().unwrap_or(1) as u8)
}

pub(crate) fn print_usage() {
    usage::print();
}
