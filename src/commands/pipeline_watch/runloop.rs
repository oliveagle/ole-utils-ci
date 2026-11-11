// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The interactive loop of the dashboard.
//!
//! Fetch, render, wait for the next tick or the next keystroke. Errors
//! from individual project fetches are accumulated and rendered with the
//! rows; a partial failure never blacks out the dashboard.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use chrono::{Local, Utc};

use crate::gitlab::WatchConfig;

use super::cache::Cache;
use super::client::Client;
use super::fetch;
use super::keys::{self, Key};
use super::render;
use super::rows::RepoRow;
use super::terminal;

/// How long the loop waits between checks of the keyboard.
const POLL: Duration = Duration::from_millis(100);

/// The bounds of the refresh interval.
const MIN_INTERVAL: i64 = 5;
const MAX_INTERVAL: i64 = 120;

/// The step the interval moves by.
const INTERVAL_STEP: i64 = 5;

/// What the loop keeps between ticks.
struct State {
    /// The rows to render.
    rows: Vec<RepoRow>,
    /// The selected row.
    selected: i64,
    /// The refresh interval in seconds.
    interval: i64,
    /// How many pipelines each row shows.
    per_repo: i64,
}

impl State {
    /// A fresh state from the configuration.
    fn new(config: &WatchConfig) -> State {
        State {
            rows: Vec::new(),
            selected: 0,
            interval: config.refresh.interval_seconds,
            per_repo: config.per_repo,
        }
    }

    /// Fetches one round of rows.
    fn refresh(&mut self, client: &Client, config: &WatchConfig, cache: &mut Cache) {
        let result = fetch::fetch_dashboard(client, config, cache, Utc::now());
        self.rows = result.rows;
    }

    /// Applies one key event; returns true when the dashboard should quit.
    fn handle(
        &mut self,
        key: Key,
        client: &Client,
        config: &WatchConfig,
        cache: &mut Cache,
        token_source: &str,
    ) -> Result<bool, String> {
        if key == Key::Quit {
            return Ok(true);
        }
        match key {
            Key::Refresh => self.refresh(client, config, cache),
            Key::IntervalUp => self.interval = clamp(self.interval + INTERVAL_STEP),
            Key::IntervalDown => self.interval = clamp(self.interval - INTERVAL_STEP),
            Key::Down => self.move_down(),
            Key::Up => self.move_up(),
            Key::Open => self.open_selected(),
            // A key the dashboard does not act on leaves the frame alone.
            _ => return Ok(false),
        }
        self.render(token_source)?;
        Ok(false)
    }

    /// Moves the selection one row down, stopping at the last row.
    fn move_down(&mut self) {
        let last = self.rows.len() as i64 - 1;
        if self.selected < last {
            self.selected += 1;
        }
    }

    /// Moves the selection one row up, stopping at the first row.
    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Opens the selected repo's latest pipeline in the browser.
    fn open_selected(&self) {
        if self.selected < 0 {
            return;
        }
        let Some(row) = self.rows.get(self.selected as usize) else {
            return;
        };
        if row.web_url.is_empty() {
            return;
        }
        open_browser(&row.web_url);
    }

    /// Paints one frame.
    fn render(&self, token_source: &str) -> Result<(), String> {
        let mut frame: Vec<u8> = Vec::new();
        render::render_dashboard(
            &mut frame,
            &self.rows,
            self.per_repo as usize,
            self.selected,
            self.interval,
            Local::now(),
            token_source,
        )
        .map_err(|error| format!("pipeline-watch: render: {error}"))?;
        let mut stdout = std::io::stdout();
        write_frame(&mut stdout, &frame)
            .map_err(|error| format!("pipeline-watch: write frame: {error}"))
    }
}

/// Clamps a refresh interval into the supported range.
fn clamp(value: i64) -> i64 {
    value.clamp(MIN_INTERVAL, MAX_INTERVAL)
}

/// Writes one frame: clear the screen, home the cursor, then the rows.
///
/// Raw mode turns off the newline-to-carriage-return translation, so a
/// bare newline would only move the cursor down and every row would start
/// where the previous one ended — the staircase layout. The frame is
/// rewritten to CRLF for that reason.
fn write_frame(out: &mut impl Write, frame: &[u8]) -> std::io::Result<()> {
    let text = String::from_utf8_lossy(frame);
    let body = text.replace("\r\n", "\n").replace('\n', "\r\n");
    out.write_all("\u{1b}[2J\u{1b}[H".as_bytes())?;
    out.write_all(body.as_bytes())?;
    out.flush()
}

/// Runs the interactive dashboard.
pub fn run(config: &WatchConfig, client: &Client, token_source: &str) -> Result<i32, String> {
    let raw = terminal::stdin_is_tty();
    let _mode = if raw {
        terminal::RawMode::enable()
    } else {
        None
    };
    let keys: Receiver<Key> = keys::spawn_reader(raw);
    let mut state = State::new(config);
    let mut cache = Cache::new();
    state.refresh(client, config, &mut cache);
    state.render(token_source)?;
    let mut next_tick = Instant::now() + Duration::from_secs(state.interval as u64);
    loop {
        match keys.recv_timeout(POLL) {
            Ok(key) => {
                let interval = state.interval;
                if state.handle(key, client, config, &mut cache, token_source)? {
                    return Ok(0);
                }
                // A new interval restarts the countdown, the way Go's
                // `ticker.Reset` does.
                if state.interval != interval {
                    next_tick = Instant::now() + Duration::from_secs(state.interval as u64);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(0),
        }
        if Instant::now() >= next_tick {
            state.refresh(client, config, &mut cache);
            state.render(token_source)?;
            next_tick = Instant::now() + Duration::from_secs(state.interval as u64);
        }
    }
}

/// Opens a URL in the operator's browser.
///
/// The failure is silent on purpose: an unresolved browser launcher must
/// never take the dashboard down.
fn open_browser(url: &str) {
    if url.is_empty() {
        return;
    }
    let (program, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        ("rundll32", vec!["url.dll,FileProtocolHandler", url])
    } else {
        ("xdg-open", vec![url])
    };
    let _ = Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(test)]
mod tests;
