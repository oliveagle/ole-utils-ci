// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The private flag set of `pipeline-run` and the split Go's `flag`
//! package forces on it.
//!
//! Go's `flag` stops at the first non-flag argument, while the documented
//! usage is `pipeline-run <project-path> [flags]`. The Go original therefore
//! walks the argument list itself, collects `--var` by hand and only then
//! hands the flag tokens to the `FlagSet`. Both halves live here.

use crate::cli::{report_parse_error, FlagSet};

/// The `--wait-timeout` default, in seconds.
pub(super) const DEFAULT_WAIT_TIMEOUT: i64 = 600;

/// Everything one run needs after the flags are parsed.
#[derive(Debug, Clone)]
pub(super) struct RunFlags {
    /// `--ref`: branch or tag to run on.
    pub(super) reference: String,
    /// `--var KEY=VALUE`, in command-line order.
    pub(super) variables: Vec<String>,
    /// `--token`, possibly empty.
    pub(super) token: String,
    /// `--base-url`, possibly empty.
    pub(super) base_url: String,
    /// `--config`, possibly empty.
    pub(super) config: String,
    /// The effective `--output` / `-o` value.
    pub(super) output: String,
    /// `--pretty`: the historical alias for `--output=json`.
    pub(super) pretty: bool,
    /// `--wait`: poll until the pipeline finishes.
    pub(super) wait: bool,
    /// `--wait-timeout` in seconds.
    pub(super) wait_timeout: i64,
    /// `--expect-success`: with `--wait`, fail unless the result is success.
    pub(super) expect_success: bool,
}

/// The flag set `pipeline-run` registers, in Go's order and wording.
fn build_flags() -> FlagSet {
    let mut flags = FlagSet::new();
    flags.str_flag("ref", "main");
    flags.str_flag("token", "");
    flags.str_flag("base-url", "");
    flags.str_flag("config", "");
    flags.str_flag("output", "toon");
    flags.str_flag("o", "toon");
    flags.bool_flag("pretty", false);
    flags.bool_flag("wait", false);
    flags.int_flag("wait-timeout", DEFAULT_WAIT_TIMEOUT);
    flags.bool_flag("expect-success", false);
    flags.describe("ref", "branch or tag to run the pipeline on");
    flags.describe(
        "token",
        "GitLab personal access token (default: $GITLAB_TOKEN)",
    );
    flags.describe("base-url", "override GitLab base URL");
    flags.describe(
        "config",
        "config file path (default: ~/.devine/utils/ci/config.yaml)",
    );
    flags.describe("output", "output format: toon|json|text");
    flags.describe("o", "shorthand for --output");
    flags.describe("pretty", "alias for --output=json");
    flags.describe("wait", "poll until pipeline finishes");
    flags.describe(
        "wait-timeout",
        "max seconds to wait with --wait (default 600)",
    );
    flags.describe(
        "expect-success",
        "with --wait: exit non-zero unless the pipeline ends in success",
    );
    flags
}

/// One string flag's value; the flag set always holds the default.
fn text(flags: &FlagSet, name: &str) -> String {
    flags
        .get(name)
        .map(|value| value.as_str(""))
        .unwrap_or_default()
}

/// The pre-split result: what the flag set should see and what it must not.
struct Split {
    /// The flag tokens, with their values already pulled next to them.
    flag_args: Vec<String>,
    /// Everything that is not a flag.
    positional: Vec<String>,
    /// Every `--var`, in command-line order.
    variables: Vec<String>,
    /// The last `--output` / `-o` value seen. The two flags write one Go
    /// variable, so only the command line can say which one won.
    last_output: Option<String>,
}

/// `parsePipelineRunFlags`: split first, then parse the flag tokens.
pub(super) fn parse_flags(args: &[String]) -> Result<(RunFlags, Vec<String>), String> {
    let split = split_args(args);
    let mut flags = build_flags();
    if let Err(error) = flags.parse(&split.flag_args) {
        report_parse_error(&error, &flags.render_usage("pipeline-run"));
        return Err(error.to_string());
    }
    let output = split.last_output.unwrap_or_else(|| text(&flags, "output"));
    let parsed = RunFlags {
        reference: text(&flags, "ref"),
        variables: split.variables,
        token: text(&flags, "token"),
        base_url: text(&flags, "base-url"),
        config: text(&flags, "config"),
        output,
        pretty: flags.get("pretty").is_none_or(|value| value.as_bool(false)),
        wait: flags.get("wait").is_none_or(|value| value.as_bool(false)),
        wait_timeout: flags
            .get("wait-timeout")
            .map_or(DEFAULT_WAIT_TIMEOUT, |value| {
                value.as_int(DEFAULT_WAIT_TIMEOUT)
            }),
        expect_success: flags
            .get("expect-success")
            .is_none_or(|value| value.as_bool(false)),
    };
    Ok((parsed, split.positional))
}

/// The flags that take a value even when they are spelled without `=`.
const VALUE_FLAGS: [&str; 7] = [
    "ref",
    "token",
    "base-url",
    "config",
    "output",
    "o",
    "wait-timeout",
];

/// Walks the raw arguments the way the Go loop does: `--` ends the flag
/// section, `--var` is collected by hand, and a value flag written without
/// `=` swallows the token that follows it.
fn split_args(args: &[String]) -> Split {
    let mut split = Split {
        flag_args: Vec::new(),
        positional: Vec::new(),
        variables: Vec::new(),
        last_output: None,
    };
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--" {
            split.positional.extend_from_slice(&args[index + 1..]);
            break;
        }
        if arg.len() > 1 && arg.starts_with('-') {
            index = take_flag(args, index, &mut split);
            continue;
        }
        split.positional.push(arg.clone());
        index += 1;
    }
    split
}

/// Consumes the flag token at `index` and returns the next index to look
/// at — a value flag with a separate value skips that value too.
fn take_flag(args: &[String], index: usize, split: &mut Split) -> usize {
    let arg = &args[index];
    let name = arg.trim_start_matches('-');
    let (base, inline) = match name.split_once('=') {
        Some((base, value)) => (base, Some(value)),
        None => (name, None),
    };
    if base == "var" {
        // `--var` is repeatable, so it never reaches the flag set.
        if let Some(value) = inline {
            split.variables.push(value.to_string());
            return index + 1;
        }
        if let Some(value) = args.get(index + 1) {
            split.variables.push(value.clone());
            return index + 2;
        }
        return index + 1;
    }
    if matches!(base, "output" | "o") {
        if let Some(value) = inline {
            split.last_output = Some(value.to_string());
        }
    }
    split.flag_args.push(arg.clone());
    if inline.is_none() && VALUE_FLAGS.contains(&base) {
        if let Some(value) = args.get(index + 1) {
            split.flag_args.push(value.clone());
            if matches!(base, "output" | "o") {
                split.last_output = Some(value.clone());
            }
            return index + 2;
        }
    }
    index + 1
}
