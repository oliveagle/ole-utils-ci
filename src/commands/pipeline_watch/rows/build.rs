// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The aggregation that turns GitLab data into dashboard rows.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::super::client::{Pipeline, Project};
use super::{format_relative_time, sort_rows, PipelineDot, RepoRow};

/// Everything the row builder reads.
pub struct Sources<'a> {
    /// The pipelines of every project, newest first.
    pub pipelines: &'a HashMap<String, Vec<Pipeline>>,
    /// The job count of every pipeline.
    pub jobs: &'a HashMap<i64, i64>,
    /// The failed-job count of every pipeline.
    pub failed_jobs: &'a HashMap<i64, i64>,
    /// The open merge-request count of every project.
    pub mr_counts: &'a HashMap<String, i64>,
    /// The worst merge-request pipeline status of every project.
    pub mr_pipelines: &'a HashMap<String, String>,
    /// The one-day commit count of every project.
    pub commits: &'a HashMap<String, i64>,
}

/// The counters the status strip produces.
struct Dots {
    /// The cells of the strip.
    dots: Vec<PipelineDot>,
    /// How many pipelines succeeded.
    success: i64,
    /// How many pipelines failed.
    failure: i64,
    /// How many pipelines are running or queued.
    running: i64,
    /// The status of the newest merge-request pipeline.
    mr_status: String,
}

/// Builds one row per project, in the canonical order.
///
/// Only the first `dot_count` pipelines feed the strip and the counters, so
/// the trailing summary always matches what the operator sees.
pub fn build_rows(
    projects: &[Project],
    sources: &Sources<'_>,
    dot_count: usize,
    now: DateTime<Utc>,
) -> Vec<RepoRow> {
    let dot_count = if dot_count == 0 { 7 } else { dot_count };
    let mut rows: Vec<RepoRow> = projects
        .iter()
        .map(|project| build_row(project, sources, dot_count, now))
        .collect();
    sort_rows(&mut rows);
    rows
}

/// Builds the row of one project.
fn build_row(
    project: &Project,
    sources: &Sources<'_>,
    dot_count: usize,
    now: DateTime<Utc>,
) -> RepoRow {
    let path = project.path_with_namespace.clone();
    let counts = count_dots(sources, &path, dot_count);
    let mut row = RepoRow {
        project: path.clone(),
        mr_count: count_of(sources.mr_counts, &path),
        mr_pipeline_status: mr_status(sources, &path, &counts.mr_status),
        commit_count_1d: count_of(sources.commits, &path),
        pipeline_dots: counts.dots,
        success_count: counts.success,
        failure_count: counts.failure,
        running_count: counts.running,
        archived: project.archived,
        ..RepoRow::default()
    };
    if let Some(head) = sources.pipelines.get(&path).and_then(|list| list.first()) {
        fill_head(&mut row, head, sources, now);
    }
    row
}

/// Fills the "last pipeline" half of one row.
fn fill_head(row: &mut RepoRow, head: &Pipeline, sources: &Sources<'_>, now: DateTime<Utc>) {
    row.last_total_jobs = count_of_id(sources.jobs, head.id);
    row.last_failed_jobs = count_of_id(sources.failed_jobs, head.id);
    row.last_status = head.effective_status();
    row.last_ref = head.r#ref.clone();
    row.last_ago = match head.updated_at {
        Some(stamp) => format_relative_time(now, stamp),
        None => "-".to_string(),
    };
    row.last_duration = head.duration_seconds();
    row.last_updated = head.updated_at;
    row.pipeline_id = head.id;
    row.web_url = head.web_url.clone();
}

/// Turns the newest pipelines into the strip and its counters.
fn count_dots(sources: &Sources<'_>, path: &str, dot_count: usize) -> Dots {
    let mut counts = Dots {
        dots: Vec::with_capacity(dot_count),
        success: 0,
        failure: 0,
        running: 0,
        mr_status: String::new(),
    };
    let Some(pipelines) = sources.pipelines.get(path) else {
        return counts;
    };
    for (index, pipeline) in pipelines.iter().enumerate() {
        if index >= dot_count {
            break;
        }
        let status = pipeline.effective_status();
        counts.dots.push(PipelineDot {
            status: status.clone(),
            job_count: count_of_id(sources.jobs, pipeline.id),
            pipeline_id: pipeline.id,
            r#ref: pipeline.r#ref.clone(),
            updated_at: pipeline.updated_at,
        });
        // "warning" counts as a success: the required jobs did pass, but the
        // strip paints it yellow so an allow_failure regression stands out.
        match status.to_lowercase().as_str() {
            "success" | "warning" => counts.success += 1,
            "failed" => counts.failure += 1,
            "running" | "pending" => counts.running += 1,
            _ => {}
        }
        if counts.mr_status.is_empty() && pipeline.source.to_lowercase() == "merge_request_event" {
            counts.mr_status = status;
        }
    }
    counts
}

/// The merge-request pipeline status of one project.
///
/// The API value wins: it covers every open merge request, even one whose
/// pipeline did not make the newest-N list. The scanned value is the
/// fallback.
fn mr_status(sources: &Sources<'_>, path: &str, scanned: &str) -> String {
    let from_api = sources.mr_pipelines.get(path).cloned().unwrap_or_default();
    if from_api.is_empty() {
        return scanned.to_string();
    }
    from_api
}

/// One counter of one project.
fn count_of(counts: &HashMap<String, i64>, path: &str) -> i64 {
    counts.get(path).copied().unwrap_or(0)
}

/// One counter of one pipeline.
fn count_of_id(counts: &HashMap<i64, i64>, pipeline_id: i64) -> i64 {
    counts.get(&pipeline_id).copied().unwrap_or(0)
}

#[cfg(test)]
mod tests;
