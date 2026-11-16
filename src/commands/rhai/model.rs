// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The data model of the Rhai engine: errors, branch sites and coverage state.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use rhai::AST;
use serde::Serialize;

/// The probe name is deliberately outside ordinary kernel-hook naming.
pub(super) const BRANCH_PROBE: &str = "__ole_utils_rhai_condition";

#[derive(Debug, Clone)]
pub struct SourceError {
    /// Stable machine-readable rule.
    pub rule: String,
    /// One-line description.
    pub message: String,
    /// One-based line; zero when unavailable.
    pub line: usize,
    /// One-based column; zero when unavailable.
    pub col: usize,
}

/// A branch condition found in valid source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BranchSite {
    /// Stable probe id, in source order.
    pub(super) id: usize,
    /// One-based condition line.
    pub(super) line: usize,
    /// One-based condition column.
    pub(super) col: usize,
    /// Start char index of the condition text.
    pub(super) start_char: usize,
    /// End char index, exclusive, of the condition text.
    pub(super) end_char: usize,
    /// Whether the syntax is `if (…)` and needs a restored closing paren.
    pub(super) parenthesised: bool,
}

/// Line and branch observations from one file's test suite.
#[derive(Debug, Default, Clone)]
pub struct RuntimeCoverage {
    /// Executed static line numbers.
    pub lines: BTreeSet<usize>,
    /// Observed `(site, outcome)` pairs.
    pub branches: BTreeSet<(usize, bool)>,
}

impl RuntimeCoverage {
    /// Shared interior state for Rhai callbacks.
    pub(crate) fn shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::default()))
    }

    /// Records one executed line.
    pub(super) fn hit_line(&mut self, line: usize) {
        if line > 0 {
            self.lines.insert(line);
        }
    }

    /// Records one branch outcome.
    pub(super) fn hit_branch(&mut self, site: usize, taken: bool) {
        self.branches.insert((site, taken));
    }
}

/// A parsed file plus the AST used for coverage execution.
pub struct LoadedSource {
    /// Original UTF-8 text.
    pub source: String,
    /// Executable AST, with branch probes when instrumentation succeeds.
    pub runtime_ast: AST,
    /// Static executable line numbers.
    pub lines: BTreeSet<usize>,
    /// Branch conditions and their source locations.
    pub branches: Vec<BranchLocation>,
    /// Set when branch instrumentation had to fall back to the original AST.
    pub coverage_error: Option<String>,
}

/// A branch site exposed in the result payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BranchLocation {
    /// Stable probe id.
    pub id: usize,
    /// One-based line.
    pub line: usize,
    /// One-based column.
    pub col: usize,
}
