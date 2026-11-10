// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Hard quality budgets owned by the tool, not by project configuration.
//!
//! CLI flags and `.devine-utils-ci.jsonc` may only select smaller values.

/// Warning threshold for source and script files.
pub const HARD_SOURCE_WARN_LINES: usize = 400;
/// Hard limit for source and script files.
pub const HARD_SOURCE_MAX_LINES: usize = 500;
/// Warning threshold for HTML and template files.
pub const HARD_HTML_WARN_LINES: usize = 800;
/// Hard limit for HTML and template files.
pub const HARD_HTML_MAX_LINES: usize = 1200;
/// Warning threshold for YAML, JSON, CSS and SQL files.
pub const HARD_DECLARATIVE_WARN_LINES: usize = 800;
/// Hard limit for YAML, JSON, CSS and SQL files.
pub const HARD_DECLARATIVE_MAX_LINES: usize = 1000;
/// Cyclomatic complexity warning ceiling.
pub const HARD_COMPLEXITY_WARN: usize = 15;
/// Cyclomatic complexity hard ceiling.
pub const HARD_COMPLEXITY_MAX: usize = 20;
/// Floor for the line-coverage threshold.
pub const COVERAGE_FLOOR: i64 = 80;
/// Warning threshold for files in a package.
pub const WARN_PACKAGE_FILES: usize = 50;
/// Warning threshold for non-empty lines in a package.
pub const WARN_PACKAGE_LINES: usize = 5000;
/// Warning threshold for exported symbols in a package.
pub const WARN_PACKAGE_EXPORTED: usize = 100;
/// Warning threshold for import fan-out of a package.
pub const WARN_PACKAGE_IMPORTS: usize = 25;
/// Warning budget for a release artifact, in MiB.
pub const HARD_ARTIFACT_WARN_MIB: u64 = 50;
/// Hard budget for a release artifact, in MiB.
pub const HARD_ARTIFACT_MAX_MIB: u64 = 80;
