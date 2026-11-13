// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `ole-utils-tengo` — The Tengo gates of the tool.
//!
//! The binary runs one language group: `ole-utils-tengo <command>` is
//! `devine-utils-ci tengo <command>`.

use std::process::ExitCode;

fn main() -> ExitCode {
    devine_utils_ci::run::entry(Some("tengo"))
}
