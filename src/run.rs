// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The entry point every binary shares.
//!
//! The main binary runs the flat command surface; a language binary runs
//! one language group. Both split the command line the way the Go original
//! did: the first argument is the command, `--ts` turns on the timestamped
//! output, and the rest are the flags of the command.

use std::io::Write;
use std::process::ExitCode;

use crate::dispatch;
use crate::languages;
use crate::usage;

/// Runs the tool.
///
/// `default_language` is the language group a standalone binary runs: the
/// main binary passes `None`, `ole-utils-golang` passes `Some("golang")`.
pub fn entry(default_language: Option<&str>) -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (command, args, timestamped) = split_args(&argv);
    if timestamped {
        return run_timestamped(&command, &args);
    }
    match default_language {
        None => run_main(&command, &args),
        Some(lang) => run_language(lang, &command, &args),
    }
}

/// Runs the flat command surface.
fn run_main(command: &str, args: &[String]) -> ExitCode {
    if command.is_empty() {
        usage::print();
        return ExitCode::from(1);
    }
    if matches!(command, "version" | "--version") {
        println!("{}", banner(&program_name()));
        return ExitCode::from(0);
    }
    finish(dispatch::dispatch(command, args))
}

/// Runs one language group.
///
/// The command the user typed becomes the subcommand of the group, so
/// `ole-utils-golang test` runs the `test` gate of the `golang` group.
fn run_language(lang: &str, command: &str, args: &[String]) -> ExitCode {
    let Some(group) = languages::find(lang) else {
        eprintln!("Error: unknown language group: {lang}");
        return ExitCode::from(1);
    };
    if command.is_empty() {
        return finish(languages::run(group, args));
    }
    if matches!(command, "help" | "-h" | "--help") {
        usage::print_language(group);
        return ExitCode::from(0);
    }
    if matches!(command, "version" | "--version") {
        println!("{}", banner(&program_name()));
        return ExitCode::from(0);
    }
    let mut rest = Vec::with_capacity(args.len() + 1);
    rest.push(command.to_string());
    rest.extend_from_slice(args);
    finish(languages::run(group, &rest))
}

/// Turns a result into an exit code.
fn finish(result: Result<i32, String>) -> ExitCode {
    match result {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(1)
        }
    }
}

/// The name the user invoked, taken from the path of the binary.
///
/// The alias binary (`ole-utils-ci`) names itself in the version banner,
/// the way the language binaries do.
fn program_name() -> String {
    let path = std::env::args().next().unwrap_or_default();
    let name = path.rsplit(['/', '\\']).next().unwrap_or(&path);
    name.strip_suffix(".exe").unwrap_or(name).to_string()
}

/// The version banner, named after the program the user ran.
fn banner(program: &str) -> String {
    format!("{program} version {}", crate::VERSION)
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
