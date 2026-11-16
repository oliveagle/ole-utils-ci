// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Line and branch coverage status for one file.

use super::engine;
use super::report::percent;

pub(super) fn coverage_status(
    known_sites: &[usize],
    observed_sites: &[usize],
    threshold: f64,
) -> CoverageStatus {
    let known: std::collections::BTreeSet<usize> = known_sites.iter().copied().collect();
    let observed: std::collections::BTreeSet<usize> = observed_sites.iter().copied().collect();
    let missing = known.difference(&observed).copied().collect::<Vec<_>>();
    let covered_sites = known.intersection(&observed).count();
    let total_sites = known.len();
    CoverageStatus {
        covered_sites,
        missing_sites: missing,
        missing_outcomes: Vec::new(),
        percent: percent(covered_sites, total_sites),
        threshold,
        total_sites,
    }
}

/// Branch coverage is outcome coverage: every guard must execute true and false.
pub(super) fn branch_coverage_status(
    sites: &[engine::BranchLocation],
    observed: &std::collections::BTreeSet<(usize, bool)>,
    threshold: f64,
) -> CoverageStatus {
    let known: std::collections::BTreeSet<(usize, bool)> = sites
        .iter()
        .flat_map(|site| [(site.id, true), (site.id, false)])
        .collect();
    let covered = known.intersection(observed).count();
    let missing = known
        .difference(observed)
        .map(|(site, taken)| format!("{site}:{taken}"))
        .collect::<Vec<_>>();
    let missing_sites = known
        .difference(observed)
        .map(|(site, _)| *site)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    CoverageStatus {
        covered_sites: covered,
        missing_sites,
        missing_outcomes: missing,
        percent: percent(covered, known.len()),
        threshold,
        total_sites: known.len(),
    }
}

/// One coverage dimension for one file.
#[derive(Debug, Clone, serde::Serialize)]

pub(super) struct CoverageStatus {
    /// Covered line sites or branch outcomes.
    pub(super) covered_sites: usize,
    /// Missing line sites, or branch sites missing at least one outcome.
    pub(super) missing_sites: Vec<usize>,
    /// Missing `site:true` / `site:false` outcomes; empty for line coverage.
    pub(super) missing_outcomes: Vec<String>,
    pub(super) percent: f64,
    pub(super) threshold: f64,
    /// Line sites or branch outcomes.
    pub(super) total_sites: usize,
}
