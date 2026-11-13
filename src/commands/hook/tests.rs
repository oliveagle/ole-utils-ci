// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the hook policy — the branch, message and size rules.
//!
//! The end-to-end behaviour (staged files, real git hooks, real push refs)
//! lives in `tests/hook.rs`.

use super::policy::{human_bytes, parse_size, Policy, MAX_FILES_DEFAULT};

#[test]
fn the_default_policy_has_the_documented_budgets() {
    let policy = Policy::default();
    assert_eq!(policy.max_files, MAX_FILES_DEFAULT);
    assert_eq!(policy.max_file_bytes, 5 * 1024 * 1024);
    assert_eq!(policy.protected, vec!["main", "master"]);
    assert!(policy.block_secrets);
}

#[test]
fn a_typed_branch_is_accepted() {
    let policy = Policy::default();
    for branch in [
        "feat/hook-checks",
        "fix/ci-hook",
        "docs/readme",
        "chore/bump-1.2.3",
        "refactor/split_dispatch",
        "hotfix/login",
        "release/0.12",
        "ci/gitlab-2",
    ] {
        assert_eq!(policy.branch_problem(branch), None, "{branch}");
    }
}

#[test]
fn a_branch_without_a_type_is_refused() {
    let policy = Policy::default();
    for branch in [
        "my-work",
        "main",
        "FEAT/upper",
        "feat/",
        "feat/Bad Slug",
        "wip/x",
    ] {
        assert!(
            policy.branch_problem(branch).is_some(),
            "{branch} should be refused"
        );
    }
}

#[test]
fn main_and_master_are_protected() {
    let policy = Policy::default();
    assert!(policy.is_protected("main"));
    assert!(policy.is_protected("master"));
    assert!(!policy.is_protected("feat/main"));
    assert!(!policy.is_protected("release/main"));
}

#[test]
fn a_conventional_message_is_accepted() {
    let policy = Policy::default();
    for message in [
        "feat(hook): add the commit size gate",
        "fix: stop the pre-push hook blocking tags",
        "docs!: rewrite the usage block",
        "chore(deps): bump serde",
        "Revert \"feat: something\"",
        "Merge branch 'fix/x' into main",
        "fixup! feat(hook): add the commit size gate",
    ] {
        assert_eq!(policy.message_problem(message), None, "{message}");
    }
}

#[test]
fn a_message_without_a_type_is_refused() {
    let policy = Policy::default();
    for message in [
        "added a thing",
        "wip",
        "feat add a thing",
        "feature: add a thing",
        "feat: ",
        "",
    ] {
        assert!(
            policy.message_problem(message).is_some(),
            "{message:?} should be refused"
        );
    }
}

#[test]
fn a_long_subject_is_refused() {
    let policy = Policy::default();
    let message = format!("feat: {}", "x".repeat(120));
    assert!(policy.message_problem(&message).is_some());
}

#[test]
fn sizes_accept_the_usual_suffixes() {
    assert_eq!(parse_size("20").unwrap(), 20);
    assert_eq!(parse_size("512k").unwrap(), 512 * 1024);
    assert_eq!(parse_size("5m").unwrap(), 5 * 1024 * 1024);
    assert_eq!(parse_size("1g").unwrap(), 1024 * 1024 * 1024);
    assert!(parse_size("big").is_err());
}

#[test]
fn bytes_are_shown_in_the_readable_unit() {
    assert_eq!(human_bytes(512), "512 B");
    assert_eq!(human_bytes(2048), "2.0 KiB");
    assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MiB");
}
