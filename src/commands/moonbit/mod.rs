// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `moonbit-check` command: check, format, tests, coverage and complexity.
//!
//! MoonBit owns the gate: `moon check --json` is the compiler, `moon test`
//! runs the suite, and `moon coverage report -f cobertura` supplies observed
//! line coverage. The command adds the repository's complexity budget and
//! renders the same structured report as every other gate.

mod check;
mod coverage;
mod engine;
mod flags;
mod report;
mod sources;
mod stages;
mod support;
mod usage;

pub use check::moonbit_check;
