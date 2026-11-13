// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The language groups.
//!
//! Every entry points at a command the flat surface already has, so the
//! groups add no behaviour of their own: they only decide which command
//! runs and in which order the aggregate runs them.

use super::Language;

/// Every language group, in the order the usage text prints them.
pub const LANGUAGES: &[Language] = &[
    Language {
        name: "golang",
        title: "Go",
        summary: "Go toolchain, mutation testing and the Go-specific gates.",
        commands: &[
            (
                "install",
                "install",
                "Install dependencies (go mod vendor).",
            ),
            (
                "lint",
                "lint",
                "The composite lint gate: gofmt, go vet, golangci-lint.",
            ),
            ("test", "test", "Run the tests with race detection."),
            (
                "build",
                "build",
                "Build the project (go build -o /dev/null ./...).",
            ),
            ("coverage", "coverage", "Run the line-coverage gate."),
            (
                "branch-coverage",
                "branch-coverage",
                "Run the branch-coverage gate (gobco).",
            ),
            (
                "test-report",
                "test-report",
                "Generate the JUnit XML test report.",
            ),
            ("gomut", "gomut", "Run Go mutation testing."),
            (
                "version-check",
                "version-check",
                "Check the VERSION file consistency.",
            ),
            (
                "go-pin",
                "go-pin",
                "Check the .go-version pin across go.mod, CI and Dockerfile.",
            ),
            (
                "no-shell-exec",
                "no-shell-exec",
                "Reject production Go code that executes bash/sh.",
            ),
            (
                "plugin-freshness",
                "plugin-freshness",
                "Reject plugin .so files older than their source.",
            ),
            (
                "goversion",
                "goversion",
                "Check the Go version against the go.mod requirement.",
            ),
            (
                "package-size",
                "package-size",
                "Check the Go package growth budgets.",
            ),
        ],
        aggregate: &[
            "install",
            "lint",
            "test",
            "build",
            "coverage",
            "branch-coverage",
            "go-pin",
            "no-shell-exec",
            "plugin-freshness",
            "goversion",
            "package-size",
        ],
    },
    Language {
        name: "rust",
        title: "Rust",
        summary: "The Rust gates: fmt, clippy, test, build, doc, audit and the policy gates.",
        commands: &[
            ("fmt", "rust-fmt", "cargo fmt --check."),
            (
                "clippy",
                "rust-clippy",
                "cargo clippy --all-targets --all-features -- -D warnings.",
            ),
            ("test", "rust-test", "cargo test --all-targets."),
            ("build", "rust-build", "cargo build --all-targets."),
            ("doc", "rust-doc", "cargo doc --no-deps."),
            (
                "audit",
                "rust-audit",
                "Dependency advisories and licence audit.",
            ),
            ("manifest", "rust-manifest", "Cargo.toml hygiene."),
            (
                "unsafe",
                "rust-unsafe",
                "No unsafe code in production code.",
            ),
            (
                "panic",
                "rust-panic",
                "No unwrap / expect / panic! in production code.",
            ),
            ("msrv", "rust-msrv", "The declared MSRV must be supported."),
        ],
        aggregate: &["rust"],
    },
    Language {
        name: "tengo",
        title: "Tengo",
        summary: "The Tengo tool gates: unit tests, coverage and mutation testing.",
        commands: &[
            ("test", "tengo-test", "Run the Tengo tool unit tests."),
            (
                "coverage",
                "tengo-coverage",
                "Run the Tengo line and branch coverage gates.",
            ),
            (
                "mutate",
                "tengo-mutate",
                "Run the Tengo source mutation testing.",
            ),
        ],
        aggregate: &["tengo-test", "tengo-coverage", "tengo-mutate"],
    },
    Language {
        name: "proto",
        title: "Protobuf",
        summary: "The protobuf gates: lint, breaking changes and code generation.",
        commands: &[
            ("lint", "buf-lint", "Run buf lint on the proto files."),
            (
                "breaking",
                "buf-breaking",
                "Check for breaking proto changes.",
            ),
            (
                "generate",
                "buf-generate",
                "Generate code from the proto files.",
            ),
        ],
        aggregate: &["buf-lint", "buf-breaking"],
    },
    Language {
        name: "web",
        title: "Web",
        summary: "The web gate: HTML, CSS, JavaScript and TypeScript.",
        commands: &[("lint", "web-lint", "Lint HTML/CSS/JS/TS/TSX/JSX.")],
        aggregate: &["web-lint"],
    },
    Language {
        name: "common",
        title: "Common",
        summary: "The language-agnostic gates every project runs.",
        commands: &[
            (
                "bugs",
                "bugs",
                "Scan the sources for the leftover marker comments.",
            ),
            (
                "sanity",
                "sanity",
                "Check trailing whitespace and conflict markers.",
            ),
            ("license", "license", "Check the license headers."),
            ("structure", "structure", "Check the directory structure."),
            (
                "file-length",
                "file-length",
                "Check the source file budgets.",
            ),
            (
                "complexity",
                "complexity",
                "Check the cyclomatic complexity.",
            ),
            (
                "arch",
                "arch",
                "Check the architecture rules (cycles, layers, naming).",
            ),
            (
                "artifact-size",
                "artifact-size",
                "Check the release-artifact budgets.",
            ),
            (
                "hook",
                "hook",
                "The commit and push policy checks a git hook runs.",
            ),
        ],
        aggregate: &[
            "bugs",
            "sanity",
            "license",
            "structure",
            "file-length",
            "complexity",
            "arch",
            "artifact-size",
        ],
    },
];
