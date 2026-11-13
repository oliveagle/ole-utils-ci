// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `ole-utils-ci` — the whole tool under the repository's own name.
//!
//! Every command the main binary has is reachable here, plus the
//! language groups.

use std::process::ExitCode;

fn main() -> ExitCode {
    devine_utils_ci::run::entry(None)
}
