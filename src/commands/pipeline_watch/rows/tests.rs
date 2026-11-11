// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the dashboard rows.

use std::collections::HashMap;

use chrono::{Duration, TimeZone, Utc};

use super::*;

fn at(seconds: i64) -> Option<DateTime<Utc>> {
    Some(Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap() + Duration::seconds(seconds))
}

fn row(project: &str) -> RepoRow {
    RepoRow {
        project: project.to_string(),
        ..RepoRow::default()
    }
}

#[test]
fn a_fresh_row_sorts_before_a_stale_one() {
    let mut rows = vec![
        RepoRow {
            project: "b".to_string(),
            last_updated: at(0),
            ..RepoRow::default()
        },
        RepoRow {
            project: "a".to_string(),
            last_updated: at(100),
            ..RepoRow::default()
        },
    ];
    sort_rows(&mut rows);
    assert_eq!(rows[0].project, "a");
    assert_eq!(rows[1].project, "b");
}

#[test]
fn an_archived_row_sinks_below_every_live_row() {
    let mut rows = vec![
        RepoRow {
            project: "archived".to_string(),
            archived: true,
            last_updated: at(100),
            ..RepoRow::default()
        },
        RepoRow {
            project: "live".to_string(),
            last_updated: at(0),
            ..RepoRow::default()
        },
    ];
    sort_rows(&mut rows);
    assert_eq!(rows[0].project, "live");
    assert_eq!(rows[1].project, "archived");
}

#[test]
fn the_tie_breakers_are_failures_then_merge_requests_then_name() {
    let mut rows = vec![
        RepoRow {
            project: "b".to_string(),
            failure_count: 1,
            ..RepoRow::default()
        },
        RepoRow {
            project: "a".to_string(),
            failure_count: 2,
            ..RepoRow::default()
        },
        RepoRow {
            project: "c".to_string(),
            failure_count: 1,
            mr_count: 4,
            ..RepoRow::default()
        },
    ];
    sort_rows(&mut rows);
    assert_eq!(rows[0].project, "a");
    assert_eq!(rows[1].project, "c");
    assert_eq!(rows[2].project, "b");
}

#[test]
fn a_row_without_pipelines_sorts_last() {
    let mut rows = vec![
        row("no-pipelines"),
        RepoRow {
            project: "with-pipelines".to_string(),
            last_updated: at(0),
            ..RepoRow::default()
        },
    ];
    sort_rows(&mut rows);
    assert_eq!(rows[0].project, "with-pipelines");
    assert_eq!(rows[1].project, "no-pipelines");
}

#[test]
fn the_relative_time_uses_the_coarsest_unit() {
    let now = Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap();
    assert_eq!(format_relative_time(now, now - Duration::seconds(5)), "5s");
    assert_eq!(format_relative_time(now, now - Duration::minutes(3)), "3m");
    assert_eq!(format_relative_time(now, now - Duration::hours(5)), "5h");
    assert_eq!(format_relative_time(now, now - Duration::hours(50)), "2d");
}

#[test]
fn a_future_stamp_carries_a_plus_sign() {
    let now = Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap();
    assert_eq!(format_relative_time(now, now + Duration::seconds(5)), "+5s");
}

#[test]
fn the_tag_annotation_fills_the_latest_column() {
    let mut rows = vec![row("devine/utils")];
    let mut tags = HashMap::new();
    tags.insert(
        "devine/utils".to_string(),
        LatestReleaseInfo {
            tag_name: "v0.3.3".to_string(),
            commit_sha: "abcdef12".to_string(),
            released_at: at(0),
            source: "releases".to_string(),
        },
    );
    annotate_latest_tags(
        &mut rows,
        &tags,
        Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap(),
    );
    assert_eq!(rows[0].latest_tag, "v0.3.3");
    assert_eq!(rows[0].latest_tag_sha, "abcdef12");
    assert_eq!(rows[0].latest_tag_ago, "0s");
}

#[test]
fn the_branch_annotation_fills_the_align_inputs() {
    let mut rows = vec![RepoRow {
        project: "devine/utils".to_string(),
        latest_tag: "v0.3.3".to_string(),
        ..RepoRow::default()
    }];
    let mut branches = HashMap::new();
    branches.insert(
        "devine/utils\u{0}main".to_string(),
        BranchHead {
            name: "main".to_string(),
            sha: "aaaa1111".to_string(),
            exists: true,
            default: true,
        },
    );
    branches.insert(
        "devine/utils\u{0}release".to_string(),
        BranchHead {
            name: "release".to_string(),
            sha: "bbbb2222".to_string(),
            exists: true,
            default: false,
        },
    );
    let mut defaults = HashMap::new();
    defaults.insert("devine/utils".to_string(), "main".to_string());
    annotate_branch_align(&mut rows, &branches, &defaults);
    assert_eq!(rows[0].main_commit_sha, "aaaa1111");
    assert_eq!(rows[0].release_commit_sha, "bbbb2222");
}

#[test]
fn a_missing_branch_leaves_the_align_inputs_empty() {
    let mut rows = vec![row("devine/utils")];
    annotate_branch_align(&mut rows, &HashMap::new(), &HashMap::new());
    assert_eq!(rows[0].main_commit_sha, "");
    assert_eq!(rows[0].release_commit_sha, "");
}
