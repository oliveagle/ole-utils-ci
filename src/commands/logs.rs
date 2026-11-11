// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `logs` command: query the OSG Clog endpoint.
//!
//! Rust port of the Go original's `internal/logs.go`. The command owns a
//! private flag set, so it receives the raw argument list and the shared
//! global parser never sees its flags.
//!
//! `-h` is special: Go's `runLogs` answers `flag.ErrHelp` by printing the
//! usage block and exiting 0, with no `Error:` line. The help path therefore
//! returns success here instead of travelling up as an error.

mod flags;
mod query;

use std::time::Duration;

use chrono::{DateTime, FixedOffset};

use crate::cli::ParseError;
use query::logs_query;

/// Everything one run needs; the defaults mirror `DefaultLogsConfig`.
#[derive(Debug, Clone)]
pub(super) struct LogsConfig {
    /// OSG application id; `100075290` is devine-fe.
    pub(super) app_id: i64,
    /// OSG token, from `--token` or `$OSG_CLOG_TOKEN`.
    pub(super) token: String,
    /// How far back to look, as a Go duration (`10m`, `1h`, `30s`).
    pub(super) from: String,
    /// End of the window; empty means "now".
    pub(super) to: String,
    /// Maximum entries to return.
    pub(super) limit: i64,
    /// Log level filter (`0` = DEBUG, `4` = FATAL).
    pub(super) level: String,
    /// Title filter, `like` syntax (`*MatchTripRecord*`).
    pub(super) title: String,
    /// The `getClogPost` endpoint.
    pub(super) url: String,
    /// Keep polling instead of running a single query.
    pub(super) follow: bool,
    /// Poll interval in nanoseconds, matching Go's `time.Duration`.
    pub(super) interval: i64,
    /// `json` prints one JSON object per entry; anything else is text.
    pub(super) output: String,
}

/// The `devine-utils-ci logs` entry point.
pub fn logs(args: &[String]) -> Result<i32, String> {
    let cfg = match flags::parse(args) {
        Ok(cfg) => cfg,
        // `runLogs` swallows `flag.ErrHelp`: the usage block is already on
        // stderr and the exit code is 0.
        Err(ParseError::Help) => return Ok(0),
        Err(error) => return Err(error.to_string()),
    };
    run(&cfg)?;
    Ok(0)
}

/// `Logs`: check the token, then query once or tail.
fn run(cfg: &LogsConfig) -> Result<(), String> {
    if cfg.token.is_empty() {
        return Err("OSG token required: set OSG_CLOG_TOKEN or pass --token".to_string());
    }
    let from = query::parse_from_flag(&cfg.from);
    let to = if cfg.to.is_empty() {
        query::now()
    } else {
        query::parse_time(&cfg.to)
    };
    if !cfg.follow {
        return logs_query(cfg, from, to);
    }
    tail(cfg, from)
}

/// Tail mode: query the window that just closed, then advance it. This never
/// returns — `--follow` is a foreground tail by design.
fn tail(cfg: &LogsConfig, from: DateTime<FixedOffset>) -> ! {
    let mut window = from;
    loop {
        let now = query::now();
        // A `--from` in the future leaves nothing to ask for yet.
        if window > now {
            sleep(cfg.interval);
            continue;
        }
        if let Err(error) = logs_query(cfg, window, now) {
            eprintln!("  ⚠ {error}");
        }
        window = now;
        sleep(cfg.interval);
    }
}

/// `time.Sleep`: a non-positive interval returns immediately.
fn sleep(nanos: i64) {
    if nanos > 0 {
        std::thread::sleep(Duration::from_nanos(nanos as u64));
    }
}

#[cfg(test)]
mod tests;
