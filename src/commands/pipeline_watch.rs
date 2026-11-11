// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `pipeline-watch`: the per-repo CI dashboard.
//!
//! Rust port of the Go original's `internal/pipeline_watch*.go`. The
//! command owns a private flag set and therefore receives the raw argument
//! list — the shared global parser must never see these flags.
//!
//! Three modes live here:
//!
//!   - `--json` prints one snapshot and exits; no terminal, no loop.
//!   - a TTY stdin gets the interactive dashboard.
//!   - anything else gets the cooked-mode loop, so a piped script still
//!     works.

mod cache;
mod client;
mod fetch;
mod flags;
mod keys;
mod local;
mod render;
mod rows;
mod runloop;
mod snapshot;
mod terminal;
mod timefmt;

#[cfg(test)]
mod tests;

use crate::gitlab;

use flags::parse_flags;

/// The entry point of `pipeline-watch`.
pub fn pipeline_watch(args: &[String]) -> Result<i32, String> {
    let flags = parse_flags(args);
    let mut config = gitlab::load_watch_config(Some(&flags.config))
        .map_err(|error| format!("pipeline-watch: load config: {error}"))?;
    if !flags.base_url.is_empty() {
        config.base_url = flags.base_url.clone();
    }
    if flags.interval > 0 {
        config.refresh.interval_seconds = flags.interval;
    }
    if flags.per_repo > 0 {
        config.per_repo = flags.per_repo;
    }
    // The flags may have pushed the values out of range; clamp again.
    config.apply_defaults();

    let (token, token_source) = gitlab::TokenSource::new(&config.token)
        .resolve()
        .map_err(|error| format!("pipeline-watch: resolve token: {error}"))?;
    let client = client::Client::new(&config.base_url, &token);

    if flags.json {
        return snapshot::write_snapshot(&config, &client, &token_source);
    }
    runloop::run(&config, &client, &token_source)
}
