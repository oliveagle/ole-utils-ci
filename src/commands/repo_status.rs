// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `repo-status`: the live pipeline status of one project.
//!
//! The dashboard is for a human watching many repositories; this command is
//! for an agent asking about one. It answers the same questions — did my last
//! push pass, what failed in pipeline N, which merge requests are open — with
//! a single JSON object and no cache, so the answer is the state of the world
//! at the moment it was printed.
//!
//! The flag set is private: `-pipelines`, `-pipeline` and `--jobs` are not
//! global flags, so the shared parser must never see them.

mod fetch;
mod flags;
mod model;
mod mrs;
mod usage;

use std::io::Write;

use chrono::Utc;
use serde::Serialize;

use crate::cli::ParseError;
use crate::commands::pipeline_watch::client::Client;
use crate::gitlab;
use crate::gojson;
use crate::output::render::encode_value;

use flags::Flags;

/// The entry point of `repo-status`.
pub fn repo_status(args: &[String]) -> Result<i32, String> {
    let (flags, positional) = match flags::parse(args) {
        Ok(parsed) => parsed,
        // Go's `flag.ExitOnError` answers `-h` with the usage block and a
        // zero exit; every other refusal gets the error line, the usage
        // block and exit 2.
        Err(ParseError::Help) => {
            eprint!("{}", usage::USAGE);
            return Ok(0);
        }
        Err(error) => {
            eprintln!("{error}");
            eprint!("{}", usage::USAGE);
            return Ok(2);
        }
    };

    let mut config = gitlab::load_watch_config(Some(&flags.config))
        .map_err(|error| format!("repo-status: load config: {error}"))?;
    if !flags.base_url.is_empty() {
        config.base_url = flags.base_url.clone();
    }
    config.apply_defaults();

    if flags.list_repos {
        return write_payload(&flags, &list_repos_payload(&config));
    }

    // Go prints the usage block and then the error, so the agent sees the
    // documented invocation right above what went wrong.
    let Some(project_path) = positional.first() else {
        eprint!("{}", usage::USAGE);
        return Err("repo-status: <project-path> required (got no positional args); pass e.g. devine/devine_utils".to_string());
    };

    let (token, token_source) = gitlab::TokenSource::new(&config.token)
        .resolve()
        .map_err(|error| format!("repo-status: resolve token: {error}"))?;
    let client = Client::new(&config.base_url, &token);
    let mut status = fetch::fetch_repo_status(&client, project_path, &flags)
        .map_err(|error| format!("repo-status {project_path}: {error}"))?;
    status.meta.token_source = token_source;
    status.meta.base_url = config.base_url.clone();
    // `time.RFC3339`, not RFC3339Nano: the fetched stamp carries whole
    // seconds.
    status.meta.fetched_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    write_payload(&flags, &status)
}

/// Encodes and prints one payload.
///
/// `--pretty` is the legacy alias for `--output=json` and wins over the mode,
/// the way `writeOutput` does it. Only the two JSON modes go through Go's
/// HTML escaper: the TOON encoder never escapes `<` in the first place.
fn write_payload(flags: &Flags, payload: &impl Serialize) -> Result<i32, String> {
    let value =
        serde_json::to_value(payload).map_err(|error| format!("repo-status: encode: {error}"))?;
    let text = encode_value(&flags.output, flags.pretty, &value)?;
    let text = if uses_json(flags) {
        gojson::to_go_json(&text)
    } else {
        text
    };
    let mut stdout = std::io::stdout();
    stdout
        .write_all(text.as_bytes())
        .map_err(|error| format!("repo-status: write: {error}"))?;
    stdout
        .flush()
        .map_err(|error| format!("repo-status: write: {error}"))?;
    Ok(0)
}

/// Whether the selected mode writes JSON as opposed to TOON.
fn uses_json(flags: &Flags) -> bool {
    if flags.pretty {
        return true;
    }
    matches!(
        flags.output.trim().to_ascii_lowercase().as_str(),
        "json" | "text"
    )
}

/// The `--list-repos` payload.
///
/// Go builds a `map[string]any`, so `encoding/json` sorts the keys; the
/// entries are inserted in that order here instead of being sorted later.
fn list_repos_payload(config: &gitlab::WatchConfig) -> serde_json::Value {
    let mut items: Vec<serde_json::Value> = Vec::new();
    for repo in &config.repos {
        if !repo.project.is_empty() {
            items.push(serde_json::json!({"kind": "project", "path": repo.project}));
        } else if !repo.group.is_empty() {
            // Go builds a `map[string]string`, and `encoding/json` sorts
            // those keys: hint, kind, path.
            items.push(serde_json::json!({
                "hint": "group — expand via `curl /api/v4/groups/<path>/projects` or ask the operator for a specific project",
                "kind": "group",
                "path": repo.group,
            }));
        }
    }
    serde_json::json!({
        "base_url": config.base_url,
        "hint": "call `devine-utils-ci repo-status <path> --pretty` with a project path from repos[].path (or a project inside a group)",
        "repos": items,
    })
}

#[cfg(test)]
mod tests;
