// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the command table.

use super::*;

/// Every string literal in a source file.
fn quoted_names(relative: &str) -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    let text = std::fs::read_to_string(path).expect("source read");
    text.split('"')
        .skip(1)
        .step_by(2)
        .map(|part| part.to_string())
        .collect()
}

#[test]
fn every_flat_command_is_named_in_the_source() {
    // The list and the match arms are written by hand, so they can drift.
    // Every name in the list must appear as a literal in the dispatcher or
    // in the Rust-gate table.
    let mut known = quoted_names("src/dispatch/mod.rs");
    known.extend(quoted_names("src/commands/rust.rs"));
    for command in FLAT_COMMANDS {
        assert!(
            known.iter().any(|name| name == command),
            "{command} is in FLAT_COMMANDS but nowhere in the dispatcher"
        );
    }
}

#[test]
fn the_flat_command_names_are_unique() {
    let mut names = FLAT_COMMANDS.to_vec();
    names.sort_unstable();
    let count = names.len();
    names.dedup();
    assert_eq!(names.len(), count, "a flat command is listed twice");
}

#[test]
fn only_the_rust_group_owns_a_flat_command_name() {
    // `rust` is the one group whose name is also a flat command: the flat
    // one wins when the caller does not name a subcommand.
    for lang in crate::languages::LANGUAGES {
        if has_flat_command(lang.name) {
            assert_eq!(lang.name, "rust", "{} shadows a flat command", lang.name);
        }
    }
}
