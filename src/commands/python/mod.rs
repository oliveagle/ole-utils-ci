// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The Python-specific subcommands.
//!
//! Two kinds of gate live here, and the difference matters:
//!
//! * the **toolchain gates** (`lint`, `format`, `typecheck`, `test`,
//!   `coverage`, `build`) drive the project's own tools. The tool is a
//!   project decision — a virtualenv's `ruff`, a pinned `mypy` — so each one
//!   is looked up on `PATH`, may be overridden with `--tool`, and reports by
//!   name when it is missing. A gate whose tool never ran is an error, never
//!   a pass.
//! * the **source gates** (`no-shell-exec`, `deps`) read the files directly.
//!   They need no interpreter, so they run in any CI job and in every job that
//!   has no Python at all.
//!
//! The interpreter is `python3` unless `--interpreter` says otherwise.

pub mod hygiene;
mod tools;

use crate::cli::FlagSet;
use crate::commands::parse_global;
use crate::config::Config;

/// The name of every Python subcommand.
pub const COMMANDS: [&str; 9] = [
    "python",
    "python-lint",
    "python-format",
    "python-typecheck",
    "python-test",
    "python-coverage",
    "python-build",
    "python-no-shell-exec",
    "python-deps",
];

/// The gates `python all` runs, in order: the two that need no toolchain
/// first, so a job with no Python still reports on the sources and the pins
/// before a missing tool can stop the run.
pub const STEPS: [&str; 7] = [
    "python-no-shell-exec",
    "python-deps",
    "python-format",
    "python-lint",
    "python-typecheck",
    "python-test",
    "python-build",
];

/// Whether the project declares itself a Python project.
///
/// One of the four files that name a Python project is enough: any of them
/// means the toolchain gates have something to drive.
pub fn is_project(root: &std::path::Path) -> bool {
    [
        "pyproject.toml",
        "setup.py",
        "setup.cfg",
        "requirements.txt",
    ]
    .iter()
    .any(|name| root.join(name).exists())
}

/// The interpreter used when the caller names none.
const DEFAULT_INTERPRETER: &str = "python3";

/// One parsed Python command line.
#[derive(Debug, Clone)]
pub struct Options {
    /// What the gate runs over; the project root when the caller names none.
    pub paths: Vec<String>,
    /// The linter / formatter / type checker to run, when it is not the default.
    pub tool: String,
    /// The interpreter that runs `-m pytest` and friends.
    pub interpreter: String,
    /// A tool configuration file, passed as `--config` when set.
    pub config: String,
    /// Extra arguments appended to the tool's own.
    pub tool_args: Vec<String>,
}

// `interpreter` defaults to `python3`, not to an empty string, so this is the
// one field `#[derive(Default)]` cannot supply.
#[allow(clippy::derivable_impls)]
impl Default for Options {
    fn default() -> Self {
        Options {
            paths: Vec::new(),
            tool: String::new(),
            interpreter: DEFAULT_INTERPRETER.to_string(),
            config: String::new(),
            tool_args: Vec::new(),
        }
    }
}

impl Options {
    /// The paths to run over: what the caller named, else the whole project.
    pub fn paths(&self) -> Vec<String> {
        if self.paths.is_empty() {
            vec![".".to_string()]
        } else {
            self.paths.clone()
        }
    }

    /// The program to run for a gate whose default is `default`.
    fn program(&self, default: &str) -> String {
        if self.tool.is_empty() {
            default.to_string()
        } else {
            self.tool.clone()
        }
    }
}

/// Adds the Python-only flags to a flag set that already holds the globals.
fn add_flags(fs: &mut FlagSet) {
    fs.value_flag("paths")
        .str_flag("tool", "")
        .str_flag("interpreter", DEFAULT_INTERPRETER)
        .str_flag("config", "")
        .value_flag("tool-args")
        .describe("paths", "paths to run over (default: the project root)")
        .describe("tool", "the linter/formatter/type checker to run")
        .describe(
            "interpreter",
            "the interpreter that runs -m pytest (default: python3)",
        )
        .describe("config", "the tool's configuration file")
        .describe("tool-args", "extra arguments appended to the tool's own");
}

/// Reads the Python flags back out of the set.
fn options_from(fs: &FlagSet, rest: Vec<String>) -> Options {
    let text = |name: &str, default: &str| {
        fs.get(name)
            .map(|value| value.as_str(default))
            .unwrap_or_else(|| default.to_string())
    };
    let mut paths = fs
        .get("paths")
        .map(|value| value.as_list().to_vec())
        .unwrap_or_default();
    if paths.is_empty() {
        paths = rest;
    }
    let interpreter = text("interpreter", DEFAULT_INTERPRETER);
    Options {
        paths,
        tool: text("tool", ""),
        // An empty `--interpreter` means "the default", not "run the empty path".
        interpreter: if interpreter.is_empty() {
            DEFAULT_INTERPRETER.to_string()
        } else {
            interpreter
        },
        config: text("config", ""),
        tool_args: fs
            .get("tool-args")
            .map(|value| value.as_list().to_vec())
            .unwrap_or_default(),
    }
}

/// Parses the flags of a Python subcommand.
pub fn parse(cfg: &mut Config, args: &[String]) -> Result<Options, String> {
    // No private flag belongs to a gate, so the shared parser can own the
    // whole line; the extras are registered beside the globals only.
    if !args.iter().any(|arg| is_private_flag(arg)) {
        let rest = parse_global(cfg, args)?;
        return Ok(Options {
            paths: rest,
            ..Options::default()
        });
    }
    let mut fs = crate::commands::global_flag_set(cfg);
    add_flags(&mut fs);
    let rest = cfg.parse(&mut fs, args)?;
    crate::environment::check(cfg.mode, cfg.mode_set)?;
    Ok(options_from(&fs, rest))
}

/// Whether an argument is one of the flags only this module registers.
fn is_private_flag(arg: &str) -> bool {
    const PRIVATE: [&str; 5] = ["paths", "tool", "interpreter", "config", "tool-args"];
    let name = arg.trim_start_matches('-');
    let name = name.split('=').next().unwrap_or(name);
    !arg.starts_with('-') || PRIVATE.contains(&name)
}

/// Runs one of the Python subcommands.
pub fn run(name: &str, cfg: &Config, opts: &Options) -> Result<i32, String> {
    match name {
        "python" => crate::commands::all::run_python(cfg),
        "python-no-shell-exec" => hygiene::no_shell_exec(cfg),
        "python-deps" => hygiene::dependencies(cfg),
        "python-lint" => tools::ruff_check(cfg, opts),
        "python-format" => tools::ruff_format(cfg, opts),
        "python-typecheck" => tools::mypy(cfg, opts),
        "python-test" => tools::pytest(cfg, opts, false),
        "python-coverage" => tools::pytest(cfg, opts, true),
        "python-build" => tools::build(cfg, opts),
        other => Err(format!("unknown Python gate: {other}")),
    }
}
