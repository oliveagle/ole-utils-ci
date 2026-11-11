// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `devine-utils-ci` — the unified CI quality-gate tool.
//!
//! This crate is the Rust rewrite of the Go `devine-utils-ci` binary. It keeps
//! the same command surface, the same `--output text|json|toon` contract and
//! the same exit-code semantics, and adds the Rust-specific gates the Go tool
//! never had (see [`rustgate`]).

pub mod cli;
pub mod commands;
pub mod config;
pub mod fsutil;
pub mod gates;
pub mod git;
pub mod gitlab;
pub mod gojson;
pub mod goparse;
pub mod httpd;
pub mod lint_config;
pub mod output;
pub mod process;
pub mod quality_config;
pub mod runner;
pub mod rustgate;
pub mod static_regex;
pub mod version;
pub mod xml;
pub mod zip;

/// Version of the tool. Kept in sync with the repository `VERSION` file and
/// injected at release time by `bump-version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Prints the version banner used by the `version` subcommand.
pub fn version_banner() -> String {
    format!("devine-utils-ci version {VERSION}")
}
