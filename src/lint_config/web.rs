// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `lint.web` block: the HTML/CSS/JS/TS syntax pass.
//!
//! The pass is off unless a project asks for it. When it is on and no path
//! is named, the whole project is linted — the same default the Go original
//! applies.

use serde::Deserialize;

/// `lint.web` as the project file spells it.
#[derive(Debug, Default, Deserialize)]
pub struct WebConfig {
    /// Explicit on/off switch; `None` keeps the default (off).
    #[serde(default)]
    pub enabled: Option<bool>,
    /// The trees to lint; empty means the whole project.
    #[serde(default)]
    pub paths: Vec<String>,
    /// `auto`, `builtin` or `external`.
    #[serde(default, rename = "ts_engine")]
    pub ts_engine: String,
    /// The `tsconfig.json` the external engine reads.
    #[serde(default, rename = "tsconfig")]
    pub tsconfig: String,
    /// Path substrings to skip.
    #[serde(default)]
    pub ignore: Vec<String>,
    /// Cap on the files linted per run.
    #[serde(default)]
    pub max_files: i64,
    /// Cap on the size of one linted file.
    #[serde(default)]
    pub max_file_size: i64,
    /// Timeout for the external TypeScript engine.
    #[serde(default, rename = "external_timeout_sec")]
    pub timeout_seconds: i64,
    /// The severity that fails the run.
    #[serde(default)]
    pub fail_on: String,
}

/// The effective web-lint policy.
#[derive(Debug)]
pub struct ResolvedWeb {
    /// Whether the pass runs at all.
    pub enabled: bool,
    /// The trees to lint.
    pub paths: Vec<String>,
    /// `auto`, `builtin` or `external`.
    pub ts_engine: String,
    /// The `tsconfig.json` the external engine reads.
    pub tsconfig: String,
    /// Path substrings to skip.
    pub ignore: Vec<String>,
    /// Cap on the files linted per run.
    pub max_files: i64,
    /// Cap on the size of one linted file.
    pub max_file_size: i64,
    /// Timeout for the external TypeScript engine.
    pub timeout_seconds: i64,
    /// The severity that fails the run.
    pub fail_on: String,
}
