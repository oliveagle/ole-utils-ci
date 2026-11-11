// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `doctor` command: what does this machine look like to the tool?
//!
//! Rust port of the Go original's `internal/doctor.go`. The report is meant
//! to be pasted into a bug report, so it is always the same human-readable
//! text: `--output` is parsed by the shared flag set and then ignored, the
//! way the Go original ignores it. A failing check exits `1` *after* the
//! report has been printed, and a warning never fails anything.

mod checks;
mod metadata;
mod project;
mod render;
mod toolchain;

use crate::config::Config;

/// The `doctor` command.
pub fn doctor(cfg: &Config) -> Result<i32, String> {
    let results = checks::run(cfg.root());
    render::report(&results);
    if results.iter().any(checks::Check::failed) {
        Ok(1)
    } else {
        Ok(0)
    }
}

#[cfg(test)]
mod tests;
