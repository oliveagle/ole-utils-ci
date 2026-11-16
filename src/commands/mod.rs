// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The subcommand surface.
//!
//! Each command owns its own flags and returns the process exit code. The
//! shared global flags live in [`crate::config`].

pub mod all;
pub mod arch;
pub mod branch_coverage;
pub mod captain_ci;
pub mod ci;
pub mod ci_recipe;
pub mod ci_recipe_cmd;
pub mod ci_recipe_util;
pub mod cleanup;
pub mod dist;
pub mod doctor;
pub mod fake_metadata;
pub mod gates;
pub mod git_author;
pub mod git_author_cmd;
pub mod goenv;
pub mod goenv_scan;
pub mod gomut;
pub mod hook;
pub mod kms_stub;
pub mod kms_stub_cmd;
pub mod kms_stub_http;
pub mod lang_tool;
pub mod logs;
pub mod mojo;
pub mod moonbit;
pub mod pipeline_run;
pub mod pipeline_watch;
pub mod protect_set;
pub mod proto;
pub mod python;
pub mod release_cmd;
pub mod release_model;
pub mod repo_status;
pub mod reports;
pub mod rhai;
pub mod rust;
pub mod tengo;
pub mod toolchain;
pub mod upgrade;
pub mod version;
pub mod web;
pub mod workflow;

use crate::cli::FlagSet;
use crate::config::Config;

/// Builds the flag set for a command that only takes the global flags.
pub fn global_flag_set(cfg: &Config) -> FlagSet {
    let mut fs = FlagSet::new();
    cfg.register(&mut fs);
    fs
}

/// Parses the global flags and returns the leftover positional arguments.
pub fn parse_global(cfg: &mut Config, args: &[String]) -> Result<Vec<String>, String> {
    let mut fs = global_flag_set(cfg);
    let rest = cfg.parse(&mut fs, args)?;
    crate::environment::check(cfg.mode, cfg.mode_set)?;
    Ok(rest)
}

/// The exit code a command returns.
pub type ExitCode = i32;

/// Checks the release model the way Go's `validateReleaseModel` does — the
/// empty string is not a model, so running with no `--release-model` fails.
pub(crate) fn validate_release_model(model: &str) -> Result<(), String> {
    match model {
        "simple" | "strict" => Ok(()),
        other => Err(format!(
            "unknown release model {other:?} — use 'simple' or 'strict'"
        )),
    }
}
