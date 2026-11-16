// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `rhai-check` command: syntax, coverage and complexity for `.rhai`.
//!
//! The syntax checker is Rhai itself, not a guess from a regex. Coverage uses
//! Rhai's debugger to observe executable lines and a semantics-preserving
//! branch probe around `if`, `while` and `do ... while` conditions. Complexity
//! uses the same counting rules as the shared complexity gate.

mod cases;
mod check;
mod coverage;
mod engine;
mod executable;
mod execute;
mod flags;
mod instrument;
mod model;
mod report;
mod source;
#[cfg(test)]
mod tests;
mod usage;
mod validate;

pub use check::rhai_check;
