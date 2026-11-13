// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The names of the flat commands.
//!
//! The list mirrors the match arms of the dispatcher plus the two groups
//! that are handled before it (the pure-analysis gates and the Rust
//! gates). It exists so a language group can tell whether the flat surface
//! already owns a name: `rust` is both a group and a flat command, and the
//! flat one wins when the caller does not name a subcommand.

/// Every flat command the dispatcher knows.
pub const FLAT_COMMANDS: &[&str] = &[
    "all",
    "arch",
    "artifact-size",
    "branch-coverage",
    "bugs",
    "buf-breaking",
    "buf-generate",
    "buf-lint",
    "build",
    "bump-version",
    "captain-ci-test",
    "ci-go-env",
    "ci-lint",
    "ci-recipe",
    "cleanup",
    "complexity",
    "coverage",
    "doctor",
    "fake-metadata-server",
    "file-length",
    "git-author",
    "glab-ci-lint",
    "go-pin",
    "gomut",
    "hook",
    "goversion",
    "install",
    "kms-stub",
    "lint",
    "license",
    "logs",
    "no-shell-exec",
    "package-size",
    "pipeline-run",
    "pipeline-watch",
    "plugin-freshness",
    "protect-set",
    "release",
    "release-check",
    "release-finish",
    "release-model",
    "release-start",
    "repo-status",
    "report-bundle",
    "rust",
    "rust-audit",
    "rust-build",
    "rust-clippy",
    "rust-doc",
    "rust-fmt",
    "rust-manifest",
    "rust-msrv",
    "rust-panic",
    "rust-test",
    "rust-unsafe",
    "sanity",
    "structure",
    "tengo-coverage",
    "tengo-mutate",
    "tengo-test",
    "test",
    "test-report",
    "upgrade",
    "verify-dist",
    "version-check",
    "web-lint",
    "workflow",
];

/// Whether the flat surface owns a command with this name.
pub fn has_flat_command(name: &str) -> bool {
    FLAT_COMMANDS.contains(&name)
}
