// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The line budgets and the extension-to-profile mapping.

use std::collections::BTreeMap;

use crate::config::{limits, Config};

/// A line budget for one profile.
#[derive(Debug, Clone, Copy)]
pub struct LineLimit {
    /// Warning threshold.
    pub warn: usize,
    /// Hard threshold.
    pub max: usize,
}

/// The built-in profile budgets.
pub fn default_profiles() -> BTreeMap<&'static str, LineLimit> {
    let mut m = BTreeMap::new();
    m.insert(
        "source",
        LineLimit {
            warn: limits::HARD_SOURCE_WARN_LINES,
            max: limits::HARD_SOURCE_MAX_LINES,
        },
    );
    m.insert(
        "script",
        LineLimit {
            warn: limits::HARD_SOURCE_WARN_LINES,
            max: limits::HARD_SOURCE_MAX_LINES,
        },
    );
    m.insert(
        "html",
        LineLimit {
            warn: limits::HARD_HTML_WARN_LINES,
            max: limits::HARD_HTML_MAX_LINES,
        },
    );
    m.insert(
        "declarative",
        LineLimit {
            warn: limits::HARD_DECLARATIVE_WARN_LINES,
            max: limits::HARD_DECLARATIVE_MAX_LINES,
        },
    );
    m
}

/// Maps a file extension to its budget profile.
pub fn profile_for_extension(ext: &str) -> Option<&'static str> {
    Some(match ext {
        ".go" | ".proto" | ".ts" | ".tsx" | ".mts" | ".cts" | ".js" | ".jsx" | ".mjs" | ".cjs"
        | ".rs" | ".py" | ".java" | ".kt" | ".swift" | ".cs" | ".c" | ".h" | ".cc" | ".cpp"
        | ".hpp" | ".tengo" => "source",
        ".sh" | ".bash" | ".zsh" | ".ps1" | ".psm1" => "script",
        ".html" | ".htm" | ".vue" | ".svelte" => "html",
        ".yaml" | ".yml" | ".json" | ".jsonc" | ".css" | ".scss" | ".less" | ".sql" => {
            "declarative"
        }
        _ => return None,
    })
}

/// Lowers the budget, never raises it.
pub fn tighten(limit: &mut LineLimit, warn: usize, max: usize) {
    if warn > 0 && warn < limit.warn {
        limit.warn = warn;
    }
    if max > 0 && max < limit.max {
        limit.max = max;
    }
}

/// Finds the path rule that matches `rel`, preferring the longest pattern.
pub fn matching_path_rule<'a>(
    rules: &'a BTreeMap<String, crate::quality_config::QualityFileLengthRule>,
    rel: &str,
) -> Option<&'a crate::quality_config::QualityFileLengthRule> {
    let mut patterns: Vec<&String> = rules.keys().collect();
    patterns.sort_by_key(|a| std::cmp::Reverse(a.len()));
    patterns
        .into_iter()
        .find(|p| crate::quality_config::match_glob_pattern(rel, p))
        .map(|p| &rules[p])
}

/// Clamps the budget to the hard ceilings the tool owns.
pub fn clamp(limit: &mut LineLimit, profile: &str, cfg: &Config) {
    if (profile == "source" || profile == "script") && limit.max > limits::HARD_SOURCE_MAX_LINES {
        limit.max = limits::HARD_SOURCE_MAX_LINES;
    }
    if cfg.max_file_lines > 0
        && (cfg.max_file_lines_set || cfg.max_file_lines != limits::HARD_SOURCE_MAX_LINES)
    {
        if limit.max > cfg.max_file_lines {
            limit.max = cfg.max_file_lines;
        }
        if limit.warn > cfg.max_file_lines {
            limit.warn = cfg.max_file_lines;
        }
    }
}

/// The budget a profile starts from.
pub fn base_limit(profile: &str) -> LineLimit {
    default_profiles()
        .get(profile)
        .copied()
        .unwrap_or(LineLimit {
            warn: limits::HARD_SOURCE_WARN_LINES,
            max: limits::HARD_SOURCE_MAX_LINES,
        })
}
