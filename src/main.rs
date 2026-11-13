// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `devine-utils-ci` — the unified CI quality-gate tool.
//!
//! The binary is a thin shell around [`devine_utils_ci::run`]: the same
//! entry point serves the language binaries (`ole-utils-golang` and
//! friends), so the command line is split in exactly one place.

use std::process::ExitCode;

fn main() -> ExitCode {
    devine_utils_ci::run::entry(None)
}
