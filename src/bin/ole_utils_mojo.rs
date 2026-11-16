// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `ole-utils-mojo` — The Mojo gates of the tool.
//!
//! The binary runs one language group: `ole-utils-mojo <command>` is
//! `devine-utils-ci mojo <command>`.

use std::process::ExitCode;

fn main() -> ExitCode {
    devine_utils_ci::run::entry(Some("mojo"))
}
