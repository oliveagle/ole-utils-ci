// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `ole-utils-common` — The language-agnostic gates of the tool.
//!
//! The binary runs one language group: `ole-utils-common <command>` is
//! `devine-utils-ci common <command>`.

use std::process::ExitCode;

fn main() -> ExitCode {
    devine_utils_ci::run::entry(Some("common"))
}
