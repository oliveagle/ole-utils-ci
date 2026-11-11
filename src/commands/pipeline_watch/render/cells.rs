// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The body cells of the dashboard.
//!
//! Every cell renders itself to an ANSI string that is exactly as wide as
//! its column, so the row stays aligned whatever the data holds.

use super::ansi;
use crate::commands::pipeline_watch::rows::{classify_align, AlignKind, PipelineDot, RepoRow};

/// The REPO cell: the star, the archive badge and the project name.
pub(super) fn cell_repo(row: &RepoRow) -> String {
    let star = if row.starred {
        format!("{}{}{} ", ansi::YELLOW, ansi::STAR, ansi::RESET)
    } else {
        "  ".to_string()
    };
    if row.archived {
        return format!(
            "{}{}A{} {}{}{}",
            star,
            ansi::REVERSE,
            ansi::RESET,
            ansi::DIM,
            row.project,
            ansi::RESET
        );
    }
    format!("{}{}{}{}", star, ansi::WHITE_FG, row.project, ansi::RESET)
}

/// The MR/1D cell: the open merge-request count over the one-day commit
/// count, behind a dot that carries the merge-request pipeline status.
pub(super) fn cell_merge_requests(row: &RepoRow) -> String {
    let glyph = if row.mr_count == 0 {
        "\u{25cb}"
    } else {
        "\u{25cf}"
    };
    format!(
        "{}{}{} {}{}/{}{}",
        ansi::BOLD,
        ansi::dot_fg(&row.mr_pipeline_status),
        glyph,
        ansi::WHITE_FG,
        row.mr_count,
        row.commit_count_1d,
        ansi::RESET
    )
}

/// The status strip: the leading state indicator and one cell per
/// pipeline.
pub(super) fn cell_strip(row: &RepoRow, width: usize) -> String {
    let prefix = match row.pipeline_dots.first() {
        Some(head) => {
            let glyph = if head.status.eq_ignore_ascii_case("running") {
                "\u{25cf}"
            } else {
                "\u{25cb}"
            };
            format!(
                "{}{}{}{} ",
                ansi::BOLD,
                ansi::dot_fg(&head.status),
                glyph,
                ansi::RESET
            )
        }
        None => format!("{}\u{25cb}{} ", ansi::DIM, ansi::RESET),
    };
    let cells: Vec<String> = row.pipeline_dots.iter().map(render_dot).collect();
    let mut text = format!("{}{}", prefix, cells.join(" "));
    let visible = ansi::visible_length(&text);
    if visible < width {
        text.push_str(&" ".repeat(width - visible));
    }
    text
}

/// One cell of the strip: the job count in white on a status-coloured
/// background.
pub(super) fn render_dot(dot: &PipelineDot) -> String {
    let digit = match dot.job_count {
        count if count <= 0 => "--".to_string(),
        count if count < 10 => format!("0{count}"),
        count if count < 100 => format!("{count}"),
        _ => "99+".to_string(),
    };
    format!(
        "{}{}{}{}{}",
        ansi::dot_bg(&dot.status),
        ansi::WHITE_FG,
        ansi::BOLD,
        digit,
        ansi::RESET
    )
}

/// The JOBS cell: the failed and total job counts of the newest pipeline.
pub(super) fn cell_jobs(row: &RepoRow) -> String {
    if row.last_total_jobs <= 0 {
        return format!("{}0/0{}", ansi::DIM, ansi::RESET);
    }
    format!(
        "{}{}{}/{}{}",
        ansi::BOLD,
        ansi::WHITE_FG,
        row.last_failed_jobs,
        row.last_total_jobs,
        ansi::RESET
    )
}

/// The CI DUR cell: the wall-clock duration of the newest pipeline.
pub(super) fn cell_duration(row: &RepoRow) -> String {
    if row.last_duration <= 0.0 {
        return format!("{}-{}", ansi::DIM, ansi::RESET);
    }
    format!(
        "{}{}{}",
        duration_color(row.last_duration),
        format_duration(row.last_duration),
        ansi::RESET
    )
}

/// The colour of a pipeline duration: green under five minutes, yellow
/// under ten, red beyond.
pub(super) fn duration_color(seconds: f64) -> String {
    if seconds < 300.0 {
        return ansi::dot_fg("success");
    }
    if seconds < 600.0 {
        return ansi::dot_fg("pending");
    }
    ansi::dot_fg("failed")
}

/// A duration in `mm:ss`, or `h:mm:ss` beyond an hour.
pub(super) fn format_duration(seconds: f64) -> String {
    if seconds <= 0.0 {
        return "-".to_string();
    }
    let total = (seconds + 0.5) as i64;
    if total < 3600 {
        return format!("{:02}:{:02}", total / 60, total % 60);
    }
    format!(
        "{}:{:02}:{:02}",
        total / 3600,
        (total % 3600) / 60,
        total % 60
    )
}

/// The LAST PIPE cell: the short status and the ref of the newest
/// pipeline.
pub(super) fn cell_last_pipeline(row: &RepoRow) -> String {
    if row.last_status.is_empty() {
        return "-".to_string();
    }
    format!(
        "{}{}{}  {}{}{}",
        ansi::BOLD,
        ansi::dot_fg(&row.last_status),
        abbreviate_status(&row.last_status),
        ansi::WHITE_FG,
        ansi::truncate_visible(&row.last_ref, 9),
        ansi::RESET
    )
}

/// The four-character short form of a pipeline status.
pub(super) fn abbreviate_status(status: &str) -> String {
    let short = match status.trim().to_lowercase().as_str() {
        "success" => "OK",
        "failed" => "FAIL",
        "running" => "RUN",
        "warning" => "WARN",
        "pending" => "WAIT",
        "canceled" | "cancelled" => "CANC",
        "skipped" => "SKIP",
        "manual" => "MAN",
        "created" => "NEW",
        _ => "",
    };
    if short.is_empty() {
        return status.to_uppercase();
    }
    short.to_string()
}

/// The `@` cell: how long ago the newest pipeline was touched.
pub(super) fn cell_age(row: &RepoRow) -> String {
    if row.last_ago.is_empty() {
        return format!("{}-{}", ansi::DIM, ansi::RESET);
    }
    format!("{}{}{}", ansi::WHITE_FG, row.last_ago, ansi::RESET)
}

/// The ALIGN cell: how closely the newest tag sits on the project heads.
pub(super) fn cell_align(row: &RepoRow) -> String {
    let green = "\u{1b}[38;2;46;160;67m";
    match classify_align(
        &row.main_commit_sha,
        &row.release_commit_sha,
        &row.latest_tag_sha,
    ) {
        AlignKind::Two => format!("{}{}2{}", ansi::BOLD, green, ansi::RESET),
        AlignKind::Three => format!("{}{}3{}", ansi::BOLD, green, ansi::RESET),
        AlignKind::Red => format!("{}x{}", ansi::ALIGN_RED_BADGE, ansi::RESET),
        AlignKind::Empty => format!("{}-{}", ansi::DIM, ansi::RESET),
    }
}

/// The LATEST cell: the newest release tag and its age.
pub(super) fn cell_latest(row: &RepoRow) -> String {
    if row.latest_tag.is_empty() {
        return format!("{}-{}", ansi::DIM, ansi::RESET);
    }
    let tag = format!(
        "{}{}{}{}",
        ansi::BOLD,
        ansi::YELLOW,
        row.latest_tag,
        ansi::RESET
    );
    if row.latest_tag_ago.is_empty() {
        return tag;
    }
    format!("{} {}{}{}", tag, ansi::DIM, row.latest_tag_ago, ansi::RESET)
}
