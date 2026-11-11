// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the row builder.

use std::collections::HashMap;

use chrono::{TimeZone, Utc};

use super::*;
use crate::commands::pipeline_watch::client::Pipeline;

fn project(path: &str) -> Project {
    Project {
        path_with_namespace: path.to_string(),
        default_branch: "main".to_string(),
        ..Project::default()
    }
}

fn pipeline(id: i64, status: &str, source: &str) -> Pipeline {
    Pipeline {
        id,
        status: status.to_string(),
        source: source.to_string(),
        r#ref: "main".to_string(),
        updated_at: Some(
            Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap() + chrono::Duration::seconds(id),
        ),
        ..Pipeline::default()
    }
}

#[test]
fn a_project_without_pipelines_still_gets_a_row() {
    let projects = vec![project("devine/utils")];
    let sources = Sources {
        pipelines: &HashMap::new(),
        jobs: &HashMap::new(),
        failed_jobs: &HashMap::new(),
        mr_counts: &HashMap::new(),
        mr_pipelines: &HashMap::new(),
        commits: &HashMap::new(),
    };
    let rows = build_rows(&projects, &sources, 7, Utc::now());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].project, "devine/utils");
    assert!(rows[0].pipeline_dots.is_empty());
    assert_eq!(rows[0].last_status, "");
}

#[test]
fn the_counters_follow_the_status_buckets() {
    let mut pipelines = HashMap::new();
    pipelines.insert(
        "devine/utils".to_string(),
        vec![
            pipeline(3, "success", "push"),
            pipeline(2, "warning", "push"),
            pipeline(1, "failed", "push"),
            pipeline(0, "running", "push"),
        ],
    );
    let mut jobs = HashMap::new();
    jobs.insert(3, 5);
    let mut failed = HashMap::new();
    failed.insert(3, 1);
    let sources = Sources {
        pipelines: &pipelines,
        jobs: &jobs,
        failed_jobs: &failed,
        mr_counts: &HashMap::new(),
        mr_pipelines: &HashMap::new(),
        commits: &HashMap::new(),
    };
    let rows = build_rows(
        &[project("devine/utils")],
        &sources,
        7,
        Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap(),
    );
    let row = &rows[0];
    assert_eq!(row.success_count, 2, "a warning counts as a success");
    assert_eq!(row.failure_count, 1);
    assert_eq!(row.running_count, 1);
    assert_eq!(row.pipeline_dots[0].job_count, 5);
    assert_eq!(row.last_total_jobs, 5);
    assert_eq!(row.last_failed_jobs, 1);
    assert_eq!(row.last_status, "success");
    assert_eq!(row.last_ref, "main");
}

#[test]
fn the_strip_stops_at_the_window() {
    let mut pipelines = HashMap::new();
    pipelines.insert(
        "devine/utils".to_string(),
        vec![
            pipeline(3, "success", "push"),
            pipeline(2, "success", "push"),
            pipeline(1, "success", "push"),
        ],
    );
    let sources = Sources {
        pipelines: &pipelines,
        jobs: &HashMap::new(),
        failed_jobs: &HashMap::new(),
        mr_counts: &HashMap::new(),
        mr_pipelines: &HashMap::new(),
        commits: &HashMap::new(),
    };
    let rows = build_rows(&[project("devine/utils")], &sources, 2, Utc::now());
    assert_eq!(rows[0].pipeline_dots.len(), 2);
    assert_eq!(rows[0].success_count, 2);
}

#[test]
fn the_merge_request_status_falls_back_to_the_scanned_one() {
    let mut pipelines = HashMap::new();
    pipelines.insert(
        "devine/utils".to_string(),
        vec![pipeline(1, "failed", "merge_request_event")],
    );
    let sources = Sources {
        pipelines: &pipelines,
        jobs: &HashMap::new(),
        failed_jobs: &HashMap::new(),
        mr_counts: &HashMap::new(),
        mr_pipelines: &HashMap::new(),
        commits: &HashMap::new(),
    };
    let rows = build_rows(&[project("devine/utils")], &sources, 7, Utc::now());
    assert_eq!(rows[0].mr_pipeline_status, "failed");
}

#[test]
fn the_api_status_wins_over_the_scanned_one() {
    let mut pipelines = HashMap::new();
    pipelines.insert(
        "devine/utils".to_string(),
        vec![pipeline(1, "failed", "merge_request_event")],
    );
    let mut mr_pipelines = HashMap::new();
    mr_pipelines.insert("devine/utils".to_string(), "running".to_string());
    let sources = Sources {
        pipelines: &pipelines,
        jobs: &HashMap::new(),
        failed_jobs: &HashMap::new(),
        mr_counts: &HashMap::new(),
        mr_pipelines: &mr_pipelines,
        commits: &HashMap::new(),
    };
    let rows = build_rows(&[project("devine/utils")], &sources, 7, Utc::now());
    assert_eq!(rows[0].mr_pipeline_status, "running");
}
