// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the language groups.

use super::*;

/// The command a group forwards a subcommand to.
fn resolve(name: &str, sub: &str) -> Option<String> {
    find(name)?.resolve(sub).map(|command| command.to_string())
}

/// Every flat command name the dispatcher knows.
fn known_commands() -> Vec<&'static str> {
    let mut known = crate::dispatch::FLAT_COMMANDS.to_vec();
    known.extend(crate::dispatch::GATE_COMMANDS);
    known.extend(crate::commands::rust::COMMANDS);
    known
}

#[test]
fn every_group_has_a_name_a_title_and_a_summary() {
    for lang in LANGUAGES {
        assert!(!lang.name.is_empty());
        assert!(!lang.title.is_empty());
        assert!(!lang.summary.is_empty());
        assert!(!lang.commands.is_empty());
        assert!(!lang.aggregate.is_empty());
    }
}

#[test]
fn group_names_are_unique() {
    let mut names: Vec<&str> = LANGUAGES.iter().map(|lang| lang.name).collect();
    names.sort_unstable();
    let count = names.len();
    names.dedup();
    assert_eq!(names.len(), count, "two groups share a name");
}

#[test]
fn a_subcommand_resolves_to_its_flat_command() {
    assert_eq!(resolve("golang", "test").as_deref(), Some("test"));
    assert_eq!(
        resolve("golang", "branch-coverage").as_deref(),
        Some("branch-coverage")
    );
    assert_eq!(resolve("tengo", "mutate").as_deref(), Some("tengo-mutate"));
    assert_eq!(
        resolve("proto", "breaking").as_deref(),
        Some("buf-breaking")
    );
    assert_eq!(resolve("web", "lint").as_deref(), Some("web-lint"));
    assert_eq!(resolve("common", "bugs").as_deref(), Some("bugs"));
}

#[test]
fn the_flat_command_name_is_accepted_too() {
    assert_eq!(
        resolve("rust", "rust-clippy").as_deref(),
        Some("rust-clippy")
    );
    assert_eq!(
        resolve("tengo", "tengo-test").as_deref(),
        Some("tengo-test")
    );
}

#[test]
fn an_unknown_subcommand_resolves_to_nothing() {
    assert_eq!(resolve("rust", "nope"), None);
    assert_eq!(resolve("golang", "clippy"), None);
    assert_eq!(resolve("nope", "test"), None);
}

#[test]
fn every_group_command_is_a_command_the_dispatcher_knows() {
    let known = known_commands();
    for lang in LANGUAGES {
        for (sub, command, description) in lang.commands {
            assert!(
                known.contains(command),
                "{}: {sub} forwards to {command}, which the dispatcher does not know",
                lang.name
            );
            assert!(
                !description.is_empty(),
                "{}: {sub} has no description",
                lang.name
            );
        }
    }
}

#[test]
fn the_binary_name_follows_the_group_name() {
    let group = find("golang").expect("group");
    assert_eq!(binary_name(group), "ole-utils-golang");
    assert_eq!(
        find("rust").map(binary_name).as_deref(),
        Some("ole-utils-rust")
    );
}

#[test]
fn the_aggregate_commands_are_listed_in_order() {
    let group = find("rust").expect("group");
    assert_eq!(group.aggregate_commands(), ["rust"]);
    let group = find("web").expect("group");
    assert_eq!(group.aggregate_commands(), ["web-lint"]);
}

#[test]
fn every_aggregate_entry_is_a_command_the_dispatcher_knows() {
    let known = known_commands();
    for lang in LANGUAGES {
        for command in lang.aggregate_commands() {
            assert!(
                known.contains(command),
                "{}: the aggregate runs {command}, which the dispatcher does not know",
                lang.name
            );
        }
    }
}

#[test]
fn a_flat_command_is_known_by_the_subcommand_that_forwards_to_it() {
    let group = find("tengo").expect("group");
    assert_eq!(group.label("tengo-test"), "test");
    assert_eq!(group.label("tengo-mutate"), "mutate");
    let group = find("proto").expect("group");
    assert_eq!(group.label("buf-lint"), "lint");
    let group = find("web").expect("group");
    assert_eq!(group.label("web-lint"), "lint");
}
