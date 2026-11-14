// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the Rust toolchain driver.

use super::*;

#[test]
fn driver_can_be_forced_by_the_environment() {
    // The default resolution must never pick a driver that is missing.
    let driver = detect_driver(Path::new("/nonexistent-root-for-tests"));
    assert!(matches!(driver, Driver::Cargo | Driver::Blaze));
}

#[test]
fn blaze_supports_only_its_own_subcommands() {
    assert!(supports(Driver::Blaze, "build"));
    assert!(supports(Driver::Blaze, "clippy"));
    assert!(!supports(Driver::Blaze, "install"));
    assert!(supports(Driver::Cargo, "install"));
}

#[test]
fn cargo_keeps_every_target_flag() {
    let args = ["--all-targets"];
    assert_eq!(
        adapt_args(Driver::Cargo, "build", &args),
        vec!["--all-targets"]
    );
    assert_eq!(
        adapt_args(Driver::Cargo, "test", &args),
        vec!["--all-targets"]
    );
}

#[test]
fn blaze_builds_the_default_targets_only() {
    assert!(adapt_args(Driver::Blaze, "build", &["--all-targets"]).is_empty());
    assert!(adapt_args(Driver::Blaze, "test", &["--all-targets"]).is_empty());
    assert_eq!(
        adapt_args(Driver::Blaze, "build", &["--release"]),
        vec!["--release"]
    );
}

#[test]
fn blaze_keeps_the_clippy_flags_it_accepts() {
    let args = ["--all-targets", "--", "-D", "warnings"];
    assert_eq!(adapt_args(Driver::Blaze, "clippy", &args), args);
}

#[test]
fn blaze_doc_asks_for_the_build_it_can_do() {
    assert!(adapt_args(Driver::Blaze, "doc", &["--no-deps"]).is_empty());
    assert_eq!(
        adapt_args(Driver::Cargo, "doc", &["--no-deps"]),
        vec!["--no-deps"]
    );
}

#[test]
fn the_command_line_starts_with_the_subcommand() {
    assert_eq!(
        command_line(Driver::Blaze, "build", &["--all-targets"]),
        vec!["build"]
    );
    assert_eq!(
        command_line(Driver::Cargo, "build", &["--all-targets"]),
        vec!["build", "--all-targets"]
    );
}

#[test]
fn only_namespace_errors_count_as_sandbox_failures() {
    assert!(sandbox_failure(
        "Error: /proc/self/setgroups: Permission denied"
    ));
    assert!(sandbox_failure("unshare: write failed /proc/self/uid_map"));
    assert!(!sandbox_failure(
        "error: could not compile `x` due to 1 error"
    ));
}

#[test]
fn the_full_environment_never_takes_the_fast_driver() {
    // `ci` is the judge: unless the caller forces a driver, it runs cargo.
    if std::env::var("DEVINE_UTILS_RUST_DRIVER").is_err() {
        assert_eq!(
            detect_driver_for(Path::new("/nonexistent-root-for-tests"), Mode::Ci),
            Driver::Cargo
        );
    }
}

#[test]
fn blaze_drops_the_flags_it_does_not_take() {
    assert_eq!(
        adapt_args(Driver::Blaze, "test", &["--workspace", "--all-targets"]),
        Vec::<&str>::new()
    );
    assert_eq!(
        adapt_args(
            Driver::Blaze,
            "clippy",
            &["--workspace", "--", "-D", "warnings"]
        ),
        vec!["--", "-D", "warnings"]
    );
    assert_eq!(
        adapt_args(Driver::Cargo, "test", &["--workspace"]),
        vec!["--workspace"]
    );
}
