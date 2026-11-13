// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `ole-utils-proto` — The Protobuf gates of the tool.
//!
//! The binary runs one language group: `ole-utils-proto <command>` is
//! `devine-utils-ci proto <command>`.

use std::process::ExitCode;

fn main() -> ExitCode {
    devine_utils_ci::run::entry(Some("proto"))
}
