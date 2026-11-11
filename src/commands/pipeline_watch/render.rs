// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The ANSI renderer of the per-repo dashboard.
//!
//! Layout, one repo per row:
//!
//! ```text
//! REPO │ MR/1D │ PIPELINE STATUS STRIP │ JOBS │ CI DUR │ LAST PIPE │ @ │ AL │ LATEST
//! ```
//!
//! Fetch errors surface as a trailing block, so the dashboard never goes
//! silently blank.

mod ansi;
mod cells;
mod columns;

use std::io::Write;

use chrono::{DateTime, Local, Timelike};

use super::rows::RepoRow;
use cells::{
    cell_age, cell_align, cell_duration, cell_jobs, cell_last_pipeline, cell_latest,
    cell_merge_requests, cell_repo, cell_strip,
};
use columns::{columns, join_row, row_width, Col};

/// Writes the per-repo dashboard to `out`.
pub fn render_dashboard(
    out: &mut impl Write,
    rows: &[RepoRow],
    dot_count: usize,
    selected: i64,
    interval: i64,
    now: DateTime<Local>,
    token_source: &str,
) -> std::io::Result<()> {
    let selected = clamp_selected(selected, rows.len());
    let dot_count = if dot_count == 0 { 7 } else { dot_count };
    let cols = columns(dot_count);

    writeln!(out, "{}", header(interval, now, token_source))?;
    writeln!(out, "{}", status_bar())?;
    let header_row = join_row(&cols, &|index| cols[index].title.to_string());
    writeln!(
        out,
        "{}{}{}{}",
        ansi::BOLD,
        ansi::WHITE_FG,
        header_row,
        ansi::RESET
    )?;
    writeln!(
        out,
        "{}{}{}",
        ansi::DIM,
        "\u{2500}".repeat(row_width(&cols)),
        ansi::RESET
    )?;

    for (index, row) in rows.iter().enumerate() {
        let is_selected = index as i64 == selected;
        writeln!(out, "{}", render_row(row, &cols, is_selected))?;
    }
    if rows.is_empty() {
        writeln!(
            out,
            "{}  (no repos — check config){}",
            ansi::DIM,
            ansi::RESET
        )?;
    }
    Ok(())
}

/// Keeps the selected index inside the row list.
fn clamp_selected(selected: i64, len: usize) -> i64 {
    if selected < 0 {
        return 0;
    }
    if len == 0 {
        return -1;
    }
    if selected >= len as i64 {
        return len as i64 - 1;
    }
    selected
}

/// The title line.
fn header(interval: i64, now: DateTime<Local>, token_source: &str) -> String {
    format!(
        "{}{}devine-utils-ci dashboard{} {}{}v{}{}  {}token={}{}  interval={}s  {}{:02}:{:02}:{:02}{}",
        ansi::BOLD,
        ansi::WHITE_FG,
        ansi::RESET,
        ansi::BOLD,
        ansi::CYAN,
        crate::VERSION,
        ansi::RESET,
        ansi::DIM,
        token_source,
        ansi::RESET,
        interval,
        ansi::DIM,
        now.hour(),
        now.minute(),
        now.second(),
        ansi::RESET
    )
}

/// The key-binding line under the title.
fn status_bar() -> String {
    let chip = |label: &str| format!("{}[{}]{}", ansi::REVERSE, label, ansi::RESET);
    format!(
        "{} quit  {} refresh  {} interval  {} select  {} open  {} star  {} logs",
        chip("q"),
        chip("r"),
        chip("+/-"),
        chip("j/k \u{2191}/\u{2193}"),
        chip("o"),
        chip("f"),
        chip("enter")
    )
}

/// Renders one body row.
fn render_row(row: &RepoRow, cols: &[Col], selected: bool) -> String {
    let strip_width = cols[2].width;
    let cells = join_row(cols, &|index| cell_for(row, index, strip_width));
    if !selected {
        return cells;
    }
    // Re-apply the selection background after every reset, so a per-cell
    // reset does not clear the row background mid-line.
    let patched = cells.replace(ansi::RESET, &format!("{}{}", ansi::RESET, ansi::SEL_BG));
    format!("{}{}{}", ansi::SEL_BG, patched, ansi::RESET)
}

/// Renders one cell of a body row.
fn cell_for(row: &RepoRow, index: usize, strip_width: usize) -> String {
    match index {
        0 => cell_repo(row),
        1 => cell_merge_requests(row),
        2 => cell_strip(row, strip_width),
        3 => cell_jobs(row),
        4 => cell_duration(row),
        5 => cell_last_pipeline(row),
        6 => cell_age(row),
        7 => cell_align(row),
        8 => cell_latest(row),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests;
