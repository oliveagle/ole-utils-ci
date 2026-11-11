// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `--json` one-shot snapshot.
//!
//! The snapshot exists so a CI script can consume the same data shape the
//! dashboard shows without dragging in a terminal. The cache is ephemeral:
//! a one-shot script must not pollute anything.

use std::io::Write;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::gitlab::WatchConfig;

use super::cache::Cache;
use super::client::Client;
use super::fetch;
use super::rows::RepoRow;

/// The snapshot the `--json` flag prints.
#[derive(Debug, Serialize)]
struct Snapshot<'a> {
    /// When the snapshot was taken.
    #[serde(serialize_with = "super::timefmt::go_stamp")]
    generated_at: DateTime<Utc>,
    /// Where the token came from.
    token_source: &'a str,
    /// The GitLab instance the rows came from.
    base_url: &'a str,
    /// The local clone root the divergence column was read from.
    local_root: &'a str,
    /// How many pipelines each row shows.
    per_repo: i64,
    /// The rows themselves.
    rows: &'a [RepoRow],
    /// The errors collected during the fetch.
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    errors: &'a [String],
}

/// Fetches one round and prints it as JSON.
pub fn write_snapshot(
    config: &WatchConfig,
    client: &Client,
    token_source: &str,
) -> Result<i32, String> {
    let text = snapshot_text(config, client, token_source)?;
    let mut stdout = std::io::stdout();
    stdout
        .write_all(text.as_bytes())
        .map_err(|error| format!("pipeline-watch: write snapshot: {error}"))?;
    stdout
        .flush()
        .map_err(|error| format!("pipeline-watch: write snapshot: {error}"))?;
    Ok(0)
}

/// Fetches one round and renders it as the JSON snapshot text.
pub fn snapshot_text(
    config: &WatchConfig,
    client: &Client,
    token_source: &str,
) -> Result<String, String> {
    let mut cache = Cache::new();
    let result = fetch::fetch_dashboard(client, config, &mut cache, Utc::now());
    let snapshot = Snapshot {
        generated_at: Utc::now(),
        token_source,
        base_url: &config.base_url,
        local_root: &config.local_root,
        per_repo: config.per_repo,
        rows: &result.rows,
        errors: &result.errors,
    };
    let text = serde_json::to_string_pretty(&snapshot)
        .map_err(|error| format!("pipeline-watch: encode snapshot: {error}"))?;
    // Go writes the snapshot with `encoding/json`, which escapes the
    // HTML-significant bytes and the two Unicode line separators.
    let mut text = crate::gojson::to_go_json(&text);
    text.push('\n');
    Ok(text)
}

#[cfg(test)]
mod tests;
