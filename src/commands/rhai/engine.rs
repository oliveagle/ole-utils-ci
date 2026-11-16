// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The Rhai execution engine behind `rhai-check`.
//!
//! Parsing is delegated to the real Rhai compiler. For coverage, branch
//! conditions are wrapped in a native probe that returns the same boolean, and
//! Rhai's debugger reports every executable AST position. The original source
//! is never rewritten to disk, and scripts run with the same default sandbox
//! plus an explicit operation budget.
//!
//! The pieces live in sibling modules: [`super::model`] holds the types,
//! [`super::source`] parses and runs, [`super::executable`] finds executable
//! lines, and [`super::instrument`] rewrites branch conditions in memory.

pub use super::model::{BranchLocation, LoadedSource, RuntimeCoverage, SourceError};
#[cfg(test)]
pub use super::source::parse;
pub use super::source::{call_hook, coverage_engine, load, resolve_source, to_json};
