// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `ole-utils-rust` — The Rust gates of the tool.
//!
//! The binary runs one language group: `ole-utils-rust <command>` is
//! `devine-utils-ci rust <command>`.

use std::process::ExitCode;

fn main() -> ExitCode {
    devine_utils_ci::run::entry(Some("rust"))
}
