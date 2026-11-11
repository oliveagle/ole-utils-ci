// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The private flag set of `repo-status`.
//!
//! Go hands the argument list through `reorderFlagsFirst` before parsing, so
//! an agent may write `repo-status devine/foo --pretty` and still have the
//! flag read. The heuristic that makes this work — a non-boolean flag eats
//! the next token unless it starts with `-` — is reproduced exactly, because
//! it decides which token ends up as the project path.

use crate::cli::{FlagSet, ParseError};

/// How many pipelines the command fetches when the caller says nothing.
const DEFAULT_PIPELINES: i64 = 5;
/// How long one request may take, in seconds.
const DEFAULT_TIMEOUT: i64 = 30;
/// The narrowest and widest accepted pipeline window.
const PIPELINES_RANGE: (i64, i64) = (1, 50);

/// What `repo-status` was asked to do.
#[derive(Debug, Clone)]
pub struct Flags {
    /// The config file, or empty for the default location.
    pub config: String,
    /// A GitLab base URL that overrides the config.
    pub base_url: String,
    /// How many recent pipelines to fetch.
    pub pipelines: i64,
    /// One specific pipeline id, which wins over `pipelines`.
    pub pipeline_id: i64,
    /// Whether to attach the job list.
    pub include_jobs: bool,
    /// Whether to attach the job logs, which implies `include_jobs`.
    pub include_trace: bool,
    /// Whether to list the open merge requests.
    pub mrs: bool,
    /// The legacy alias that forces pretty JSON.
    pub pretty: bool,
    /// `toon`, `json` or `text`.
    pub output: String,
    /// Whether to print the configured repos and stop.
    pub list_repos: bool,
    /// The per-request timeout, in seconds.
    pub timeout: i64,
}

impl Default for Flags {
    fn default() -> Flags {
        Flags {
            config: String::new(),
            base_url: String::new(),
            pipelines: DEFAULT_PIPELINES,
            pipeline_id: 0,
            include_jobs: false,
            include_trace: false,
            mrs: false,
            pretty: false,
            output: "toon".to_string(),
            list_repos: false,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

/// The flag set `parseRepoStatusFlags` registers.
pub(super) fn build() -> FlagSet {
    let mut flags = FlagSet::new();
    flags.str_flag("config", "");
    flags.str_flag("base-url", "");
    flags.int_flag("pipelines", DEFAULT_PIPELINES);
    flags.int_flag("pipeline", 0);
    flags.bool_flag("jobs", false);
    flags.bool_flag("trace", false);
    flags.bool_flag("mrs", false);
    flags.bool_flag("pretty", false);
    flags.str_flag("output", "toon");
    // `-o` and `--output` write ONE Go variable, so the last one on the
    // command line wins.
    flags.str_flag("o", "toon");
    flags.alias("o", "output");
    flags.bool_flag("list-repos", false);
    flags.int_flag("timeout", DEFAULT_TIMEOUT);
    flags
}

/// Parses `argv` into flags plus the positional arguments.
pub(super) fn parse(argv: &[String]) -> Result<(Flags, Vec<String>), ParseError> {
    let mut set = build();
    set.parse(&reorder_flags_first(argv))?;
    let positional = set.args.clone();
    Ok((read(&set), positional))
}

/// Reads the parsed values back out of the set.
fn read(set: &FlagSet) -> Flags {
    let defaults = Flags::default();
    Flags {
        config: text(set, "config"),
        base_url: text(set, "base-url"),
        pipelines: number(set, "pipelines", defaults.pipelines),
        pipeline_id: number(set, "pipeline", 0),
        include_jobs: boolean(set, "jobs"),
        include_trace: boolean(set, "trace"),
        mrs: boolean(set, "mrs"),
        pretty: boolean(set, "pretty"),
        output: text(set, "output"),
        list_repos: boolean(set, "list-repos"),
        timeout: number(set, "timeout", defaults.timeout),
    }
    .clamped()
}

impl Flags {
    /// Applies the defaults and the range limits `parseRepoStatusFlags`
    /// enforces after `fs.Parse`.
    fn clamped(self) -> Flags {
        let (low, high) = PIPELINES_RANGE;
        let mut out = self;
        out.pipelines = out.pipelines.clamp(low, high);
        if out.timeout <= 0 {
            out.timeout = DEFAULT_TIMEOUT;
        }
        if out.include_trace {
            out.include_jobs = true;
        }
        out
    }
}

/// Moves every leading positional to the end, so `flag`'s stop-at-the-first-
/// non-flag rule cannot hide a later flag.
pub(super) fn reorder_flags_first(argv: &[String]) -> Vec<String> {
    let mut flags: Vec<String> = Vec::new();
    let mut positional: Vec<String> = Vec::new();
    let mut index = 0;
    while index < argv.len() {
        let token = &argv[index];
        if !token.starts_with('-') {
            positional.push(token.clone());
            index += 1;
            continue;
        }
        flags.push(token.clone());
        let carries_value = !token.contains('=') && !is_bool_flag(token);
        let next_is_value = argv
            .get(index + 1)
            .is_some_and(|next| !next.starts_with('-'));
        if carries_value && next_is_value {
            if let Some(value) = argv.get(index + 1) {
                flags.push(value.clone());
            }
            index += 2;
            continue;
        }
        index += 1;
    }
    flags.append(&mut positional);
    flags
}

/// Whether the flag takes no value.
fn is_bool_flag(token: &str) -> bool {
    matches!(
        token.trim_start_matches('-'),
        "jobs" | "trace" | "mrs" | "pretty" | "list-repos"
    )
}

/// One string flag's value; the set always holds the default.
fn text(set: &FlagSet, name: &str) -> String {
    set.get(name)
        .map(|value| value.as_str(""))
        .unwrap_or_default()
}

/// One integer flag's value.
fn number(set: &FlagSet, name: &str, default: i64) -> i64 {
    set.get(name).map_or(default, |value| value.as_int(default))
}

/// One boolean flag's value.
fn boolean(set: &FlagSet, name: &str) -> bool {
    set.get(name)
        .map(|value| value.as_bool(false))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests;
