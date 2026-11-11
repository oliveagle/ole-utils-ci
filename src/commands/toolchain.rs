// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The subcommands that drive the Go toolchain.
//!
//! `install`, `lint`, `test`, `build`, `coverage` and `test-report` all
//! shell out to the Go toolchain, exactly as the Go original does: the
//! commands install the module dependencies first, then run the tool, and
//! word their failures the way Go words them.

pub mod build;
pub mod coverage;
pub mod covergen;
pub mod install;
pub mod junit;
pub mod lint;
pub mod testing;

pub use build::build;
pub use coverage::coverage;
pub use install::install;
pub use junit::test_report;
pub use lint::lint;
pub use testing::test;

use crate::config::limits;

/// The default coverage floor, re-exported for callers that need it.
pub fn coverage_floor() -> i64 {
    limits::COVERAGE_FLOOR
}

#[cfg(test)]
mod tests;
