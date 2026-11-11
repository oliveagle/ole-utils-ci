// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the dashboard renderer.

use chrono::{Local, TimeZone, Utc};

use super::*;
use crate::commands::pipeline_watch::rows::PipelineDot;

fn at(seconds: i64) -> Option<chrono::DateTime<Utc>> {
    Some(Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap() + chrono::Duration::seconds(seconds))
}

fn row(project: &str) -> RepoRow {
    RepoRow {
        project: project.to_string(),
        ..RepoRow::default()
    }
}

fn render(rows: &[RepoRow], selected: i64) -> String {
    let mut out: Vec<u8> = Vec::new();
    render_dashboard(
        &mut out,
        rows,
        7,
        selected,
        10,
        Local.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap(),
        "config",
    )
    .expect("render");
    String::from_utf8(out).expect("utf-8")
}

#[test]
fn the_header_names_the_version_and_the_token_source() {
    let text = render(&[row("devine/utils")], 0);
    let first = text.lines().next().unwrap_or_default().to_string();
    assert!(first.contains("devine-utils-ci dashboard"), "{first}");
    assert!(first.contains(&format!("v{}", crate::VERSION)), "{first}");
    assert!(first.contains("token=config"), "{first}");
    assert!(first.contains("interval=10s"), "{first}");
    assert!(first.contains("09:00:00"), "{first}");
}

#[test]
fn the_status_bar_lists_the_key_bindings() {
    let text = render(&[row("devine/utils")], 0);
    let second = text.lines().nth(1).unwrap_or_default().to_string();
    for binding in ["[q]", "[r]", "[+/-]", "[j/k", "[o]", "[f]", "[enter]"] {
        assert!(second.contains(binding), "{second}");
    }
}

#[test]
fn the_columns_line_up() {
    let text = render(&[row("devine/utils")], 0);
    let header = text.lines().nth(2).unwrap_or_default().to_string();
    assert!(header.contains("REPO"));
    assert!(header.contains("MR/1D"));
    assert!(header.contains("PIPELINE STATUS STRIP"));
    assert!(header.contains("JOBS"));
    assert!(header.contains("CI DUR"));
    assert!(header.contains("LAST PIPE"));
    assert!(header.contains("LATEST"));
}

#[test]
fn an_empty_dashboard_says_so() {
    let text = render(&[], 0);
    assert!(text.contains("(no repos — check config)"), "{text}");
}

#[test]
fn a_row_shows_its_project_and_counts() {
    let row = RepoRow {
        project: "devine/utils".to_string(),
        mr_count: 3,
        commit_count_1d: 12,
        last_total_jobs: 9,
        last_failed_jobs: 1,
        last_status: "success".to_string(),
        last_ref: "main".to_string(),
        last_ago: "5m".to_string(),
        last_duration: 120.0,
        latest_tag: "v0.3.3".to_string(),
        latest_tag_ago: "2d".to_string(),
        pipeline_dots: vec![PipelineDot {
            status: "success".to_string(),
            job_count: 9,
            pipeline_id: 42,
            r#ref: "main".to_string(),
            updated_at: at(0),
        }],
        ..RepoRow::default()
    };
    let text = render(&[row], 0);
    let line = text.lines().nth(4).unwrap_or_default().to_string();
    assert!(line.contains("devine/utils"), "{line}");
    assert!(line.contains("3/12"), "{line}");
    assert!(line.contains("1/9"), "{line}");
    assert!(line.contains("02:00"), "{line}");
    assert!(line.contains("OK"), "{line}");
    assert!(line.contains("v0.3.3"), "{line}");
}

#[test]
fn the_selected_row_gets_the_highlight() {
    let rows = vec![row("first"), row("second")];
    let text = render(&rows, 1);
    let lines: Vec<&str> = text.lines().collect();
    assert!(!lines[4].contains(ansi::SEL_BG), "{}", lines[4]);
    assert!(lines[5].contains(ansi::SEL_BG), "{}", lines[5]);
}

#[test]
fn an_archived_row_is_badged_and_dimmed() {
    let rows = vec![RepoRow {
        project: "mirror".to_string(),
        archived: true,
        ..RepoRow::default()
    }];
    let text = render(&rows, 0);
    let line = text.lines().nth(4).unwrap_or_default().to_string();
    assert!(line.contains(ansi::REVERSE), "{line}");
    assert!(line.contains(ansi::DIM), "{line}");
}

#[test]
fn a_starred_row_shows_the_star() {
    let rows = vec![RepoRow {
        project: "starred".to_string(),
        starred: true,
        ..RepoRow::default()
    }];
    let text = render(&rows, 0);
    let line = text.lines().nth(4).unwrap_or_default().to_string();
    assert!(line.contains(ansi::STAR), "{line}");
}
