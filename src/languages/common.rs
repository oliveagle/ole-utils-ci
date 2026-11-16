// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The language-agnostic group.
//!
//! The common gates belong to no language, so they live in their own file
//! and the registry names them with one line.

use super::Language;

pub const COMMON: Language = Language {
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
};
