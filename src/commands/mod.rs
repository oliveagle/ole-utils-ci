// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The subcommand surface.
//!
//! Each command owns its own flags and returns the process exit code. The
//! shared global flags live in [`crate::config`].

pub mod all;
pub mod arch;
pub mod ci;
pub mod dist;
pub mod doctor;
pub mod fake_metadata;
pub mod gates;
pub mod git_author;
pub mod git_author_cmd;
pub mod goenv;
pub mod goenv_scan;
pub mod kms_stub;
pub mod kms_stub_cmd;
pub mod kms_stub_http;
pub mod proto;
pub mod release_model;
pub mod repo;
pub mod reports;
pub mod rust;
pub mod toolchain;
pub mod version;
pub mod web;

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
    cfg.parse(&mut fs, args)
}

/// The exit code a command returns.
pub type ExitCode = i32;
