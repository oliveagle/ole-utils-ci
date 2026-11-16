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
fn only_a_group_with_its_own_aggregate_owns_a_flat_command_name() {
    // A group named like a flat command shadows it: the group wins whenever
    // the caller does not name a subcommand. That is allowed only for the
    // groups whose `all` is the flat command itself, so the shadowed name is
    // still reachable — `rust all`, `python all`, `mojo all`.
    for lang in crate::languages::LANGUAGES {
        if has_flat_command(lang.name) {
            assert_eq!(
                lang.aggregate_commands(),
                [lang.name],
                "{} shadows a flat command and cannot reach it",
                lang.name
            );
        }
    }
    // At least one group is in that position, or the rule above proves nothing.
    assert!(
        crate::languages::LANGUAGES
            .iter()
            .any(|lang| has_flat_command(lang.name)),
        "no group shares a name with a flat command"
    );
}
