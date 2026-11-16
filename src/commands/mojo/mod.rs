// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The Mojo-specific subcommands.
//!
//! Mojo ships one program that does all of it — `mojo format`, `mojo build`,
//! `mojo test`, `mojo package`, `mojo doc` — so every toolchain gate here is
//! an invocation of that single binary, looked up on `PATH` and overridable
//! with `--tool`. The binary may not be installed; the gates then say so by
//! name and fail, because a gate whose tool never ran has not passed.
//!
//! `mojo-unsafe` is the one gate that reads the sources itself. Mojo's
//! `unsafe_*` family is the language's equivalent of Rust's `unsafe`: it turns
//! off the alias-and-lifetime checking that is the reason to write Mojo
//! rather than C, so it belongs in a gate that runs where the compiler does
//! not — a review job, a pre-commit hook, a CI job with no toolchain.

pub mod usage_gate;

use crate::cli::FlagSet;
use crate::commands::lang_tool;
use crate::commands::parse_global;
use crate::config::Config;
use crate::fsutil;

/// The name of every Mojo subcommand.
pub const COMMANDS: [&str; 7] = [
    "mojo",
    "mojo-format",
    "mojo-build",
    "mojo-test",
    "mojo-package",
    "mojo-doc",
    "mojo-unsafe",
];

/// The gates `mojo all` runs, in order: the gate that reads the sources
/// first, then the toolchain ones.
pub const STEPS: [&str; 4] = ["mojo-unsafe", "mojo-format", "mojo-build", "mojo-test"];

/// Whether the project has any Mojo source at all.
///
/// A Mojo project has no manifest to look for, so the sources are the
/// declaration: one .mojo file (or its emoji extension) is a project the
/// gates can read.
pub fn is_project(root: &std::path::Path) -> bool {
    crate::fsutil::walk_files(root)
        .iter()
        .any(|entry| SOURCE_EXTENSIONS.contains(&entry.extension().as_str()))
}

/// The program every toolchain gate runs.
const DEFAULT_TOOL: &str = "mojo";

/// The install line the "tool not found" complaint carries.
const MOJO_HINT: &str =
    "install the Mojo toolchain (https://docs.modular.com/mojo/) or pass --tool";

/// The extensions a Mojo source file can have. `.🔥` is the language's own
/// alternative extension, and it is not a typo.
pub const SOURCE_EXTENSIONS: [&str; 2] = [".mojo", ".🔥"];

/// One parsed Mojo command line.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// What the gate runs over; each gate has its own default.
    pub paths: Vec<String>,
    /// The `mojo` binary to run, when it is not the one on `PATH`.
    pub tool: String,
    /// Extra arguments appended to the tool's own.
    pub tool_args: Vec<String>,
}

impl Options {
    /// The program to run.
    pub fn program(&self) -> String {
        if self.tool.is_empty() {
            DEFAULT_TOOL.to_string()
        } else {
            self.tool.clone()
        }
    }
}

/// Adds the Mojo-only flags to a flag set that already holds the globals.
fn add_flags(fs: &mut FlagSet) {
    fs.value_flag("paths")
        .str_flag("tool", "")
        .value_flag("tool-args")
        .describe("paths", "paths to run over (each gate has its own default)")
        .describe("tool", "the mojo binary to run")
        .describe("tool-args", "extra arguments appended to the tool's own");
}

/// Whether an argument is one of the flags only this module registers.
fn is_private_flag(arg: &str) -> bool {
    const PRIVATE: [&str; 3] = ["paths", "tool", "tool-args"];
    let name = arg.trim_start_matches('-');
    let name = name.split('=').next().unwrap_or(name);
    !arg.starts_with('-') || PRIVATE.contains(&name)
}

/// Parses the flags of a Mojo subcommand.
pub fn parse(cfg: &mut Config, args: &[String]) -> Result<Options, String> {
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
    let paths = fs
        .get("paths")
        .map(|value| value.as_list().to_vec())
        .unwrap_or_default();
    Ok(Options {
        paths: if paths.is_empty() { rest } else { paths },
        tool: fs
            .get("tool")
            .map(|value| value.as_str(""))
            .unwrap_or_default(),
        tool_args: fs
            .get("tool-args")
            .map(|value| value.as_list().to_vec())
            .unwrap_or_default(),
    })
}

/// Runs one of the Mojo subcommands.
pub fn run(name: &str, cfg: &Config, opts: &Options) -> Result<i32, String> {
    match name {
        "mojo" => crate::commands::all::run_mojo(cfg),
        "mojo-unsafe" => usage_gate::no_unsafe(cfg),
        "mojo-format" => format(cfg, opts),
        "mojo-build" => build(cfg, opts),
        "mojo-test" => test(cfg, opts),
        "mojo-package" => package(cfg, opts),
        "mojo-doc" => doc(cfg, opts),
        other => Err(format!("unknown Mojo gate: {other}")),
    }
}

/// The paths a toolchain gate runs over: what the caller named, else every
/// source file the project has.
fn targets(cfg: &Config, opts: &Options) -> Vec<String> {
    if !opts.paths.is_empty() {
        return opts.paths.clone();
    }
    let mut found: Vec<String> = fsutil::walk_files(cfg.root())
        .into_iter()
        .filter(|entry| SOURCE_EXTENSIONS.contains(&entry.extension().as_str()))
        .map(|entry| entry.rel)
        .collect();
    found.sort();
    found
}

/// Runs one `mojo` subcommand and turns its exit status into a verdict.
fn invoke(
    cfg: &Config,
    opts: &Options,
    gate: &str,
    verb: &str,
    extra: &[String],
    what: &str,
) -> Result<i32, String> {
    let paths = targets(cfg, opts);
    if paths.is_empty() {
        // Nothing to check is not a pass: it is a project that has no Mojo
        // sources, which the caller has to be told rather than reassured about.
        println!("  ⚠ No Mojo sources found; skipping {gate}");
        return Ok(0);
    }
    let mut args = vec![verb.to_string()];
    args.extend(paths);
    args.extend(extra.iter().cloned());
    args.extend(opts.tool_args.iter().cloned());
    let out = lang_tool::run(cfg, &opts.program(), &args, MOJO_HINT)?;
    let result = lang_tool::verdict(cfg, gate, what, &out);
    Ok(crate::gates::emit(cfg, &result))
}

/// `mojo format --check`.
fn format(cfg: &Config, opts: &Options) -> Result<i32, String> {
    invoke(
        cfg,
        opts,
        "mojo-format",
        "format",
        &["--check".to_string()],
        "already formatted",
    )
}

/// `mojo build`.
fn build(cfg: &Config, opts: &Options) -> Result<i32, String> {
    invoke(cfg, opts, "mojo-build", "build", &[], "built")
}

/// `mojo test`.
fn test(cfg: &Config, opts: &Options) -> Result<i32, String> {
    invoke(cfg, opts, "mojo-test", "test", &[], "every test passed")
}

/// `mojo package`.
///
/// The output lands where `--out` says; the default is the project root, so a
/// CI job collects one predictable artifact.
fn package(cfg: &Config, opts: &Options) -> Result<i32, String> {
    invoke(
        cfg,
        opts,
        "mojo-package",
        "package",
        &["-o".to_string(), "dist/project.mojopkg".to_string()],
        "packaged",
    )
}

/// `mojo doc`.
fn doc(cfg: &Config, opts: &Options) -> Result<i32, String> {
    invoke(
        cfg,
        opts,
        "mojo-doc",
        "doc",
        &["-o".to_string(), "target/mojo-docs".to_string()],
        "documented",
    )
}
