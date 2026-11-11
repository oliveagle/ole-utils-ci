// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The rows of the per-repo dashboard.
//!
//! The Go original keeps the row, the builder and the renderer in one big
//! file; here the row is data, the builder lives in [`build`], and the
//! renderer lives one level up.

use std::cmp::Ordering;
use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

use super::client::{BranchHead, LatestReleaseInfo};
use super::timefmt;

mod align;
mod build;

pub use align::{classify_align, release_branch_name, AlignKind};
pub use build::{build_rows, Sources};

/// One cell of the rolling status strip.
///
/// The job count is rendered as a number on a status-coloured background:
/// it answers "how big was this build" at a glance.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PipelineDot {
    /// The pipeline status.
    pub status: String,
    /// How many jobs the pipeline had.
    pub job_count: i64,
    /// The pipeline id.
    pub pipeline_id: i64,
    /// The git ref the pipeline ran on.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub r#ref: String,
    /// When the pipeline was last touched.
    #[serde(serialize_with = "timefmt::go_time")]
    pub updated_at: Option<DateTime<Utc>>,
}

/// One row of the per-repo dashboard.
///
/// The strip is ordered newest → oldest; the counters are window-scoped so
/// the trailing summary matches what the operator sees in the strip.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RepoRow {
    /// `namespace/project`.
    pub project: String,
    /// How many merge requests are open.
    pub mr_count: i64,
    /// The status of the newest merge-request pipeline.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub mr_pipeline_status: String,
    /// How many commits landed in the last 24 hours.
    pub commit_count_1d: i64,
    /// The rolling status strip, newest first.
    pub pipeline_dots: Vec<PipelineDot>,
    /// How many pipelines of the window succeeded.
    pub success_count: i64,
    /// How many pipelines of the window failed.
    pub failure_count: i64,
    /// How many pipelines of the window are running or queued.
    pub running_count: i64,
    /// How many jobs of the newest pipeline failed.
    #[serde(skip_serializing_if = "is_zero")]
    pub last_failed_jobs: i64,
    /// How many jobs the newest pipeline had.
    #[serde(skip_serializing_if = "is_zero")]
    pub last_total_jobs: i64,
    /// The status of the newest pipeline.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub last_status: String,
    /// The ref of the newest pipeline.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub last_ref: String,
    /// How long ago the newest pipeline was touched.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub last_ago: String,
    /// The wall-clock duration of the newest pipeline, in seconds.
    #[serde(
        skip_serializing_if = "is_zero_seconds",
        serialize_with = "crate::gojson::serialize_float"
    )]
    pub last_duration: f64,
    /// When the newest pipeline was touched.
    #[serde(serialize_with = "timefmt::go_time")]
    pub last_updated: Option<DateTime<Utc>>,
    /// Whether a usable local checkout was found.
    pub local_found: bool,
    /// The branch the local checkout tracks.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub local_branch: String,
    /// How many commits the local branch is ahead of origin.
    pub local_ahead: i64,
    /// How many commits the local branch is behind origin.
    pub local_behind: i64,
    /// How many commits the checked-out branch leads origin by.
    pub local_lead: i64,
    /// How many commits the newest tag is behind origin.
    pub local_tag_behind: i64,
    /// The browsable URL of the newest pipeline.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub web_url: String,
    /// The id of the newest pipeline.
    #[serde(skip_serializing_if = "is_zero")]
    pub pipeline_id: i64,
    /// Whether the operator starred this repo.
    #[serde(skip_serializing_if = "is_false")]
    pub starred: bool,
    /// Whether GitLab marked the project read-only.
    #[serde(skip_serializing_if = "is_false")]
    pub archived: bool,
    /// The newest release tag.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub latest_tag: String,
    /// How long ago the newest release was published.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub latest_tag_ago: String,
    /// The commit the newest tag points at.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub latest_tag_sha: String,
    /// The HEAD commit of the project's default branch.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub main_commit_sha: String,
    /// The HEAD commit of the release branch.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub release_commit_sha: String,
}

/// Whether a count is zero, the condition for dropping it from the JSON.
fn is_zero(value: &i64) -> bool {
    *value == 0
}

/// Whether a duration is zero, the condition for dropping it from the JSON.
fn is_zero_seconds(value: &f64) -> bool {
    *value == 0.0
}

/// Whether a flag is false, the condition for dropping it from the JSON.
fn is_false(value: &bool) -> bool {
    !*value
}

/// The canonical dashboard ordering.
///
/// Archived rows sink below every live row — a read-only mirror must not
/// read as something to act on, however fresh its history is. Within a
/// tier the classic keys apply: most-recently-updated first, then
/// failures, then merge requests, then alphabetical.
pub fn sort_rows(rows: &mut [RepoRow]) {
    rows.sort_by(|left, right| {
        if left.archived != right.archived {
            return if left.archived {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        right
            .last_updated
            .cmp(&left.last_updated)
            .then_with(|| right.failure_count.cmp(&left.failure_count))
            .then_with(|| right.mr_count.cmp(&left.mr_count))
            .then_with(|| left.project.cmp(&right.project))
    });
}

/// Fills the LATEST column from the release map.
///
/// A project with no known release keeps empty strings, so the renderer
/// shows a dim dash.
pub fn annotate_latest_tags(
    rows: &mut [RepoRow],
    tags: &HashMap<String, LatestReleaseInfo>,
    now: DateTime<Utc>,
) {
    for row in rows.iter_mut() {
        let Some(tag) = tags.get(&row.project) else {
            continue;
        };
        if tag.tag_name.is_empty() {
            continue;
        }
        row.latest_tag = tag.tag_name.clone();
        row.latest_tag_sha = tag.commit_sha.clone();
        if let Some(released) = tag.released_at {
            row.latest_tag_ago = format_relative_time(now, released);
        }
    }
}

/// Fills the ALIGN inputs from the branch-HEAD map.
///
/// The map is keyed by project path plus branch name. Only the project's
/// default branch and the strict-model release branch are consumed.
pub fn annotate_branch_align(
    rows: &mut [RepoRow],
    branches: &HashMap<String, BranchHead>,
    default_branches: &HashMap<String, String>,
) {
    for row in rows.iter_mut() {
        let default_branch = default_branches
            .get(&row.project)
            .filter(|name| !name.is_empty())
            .cloned()
            .unwrap_or_else(|| "main".to_string());
        row.main_commit_sha = head_sha(branches, &row.project, &default_branch);
        row.release_commit_sha = release_head(branches, row);
    }
}

/// The HEAD commit of one branch, if the branch exists and has a commit.
fn head_sha(branches: &HashMap<String, BranchHead>, project: &str, branch: &str) -> String {
    branches
        .get(&format!("{project}\u{0}{branch}"))
        .filter(|head| head.exists && !head.sha.is_empty())
        .map(|head| head.sha.clone())
        .unwrap_or_default()
}

/// The HEAD commit of the strict-model release branch, if the project uses
/// one.
fn release_head(branches: &HashMap<String, BranchHead>, row: &RepoRow) -> String {
    if row.latest_tag.is_empty() {
        return String::new();
    }
    let release = release_branch_name(&row.latest_tag);
    if release.is_empty() {
        return String::new();
    }
    head_sha(branches, &row.project, &release)
}

/// Formats a timestamp as a bare duration: `5s`, `2m`, `3h`, `2d`.
///
/// The " ago" suffix is left off on purpose — the column is five characters
/// wide, and a sign convention (bare for the past, a leading `+` for the
/// future) reads uniformly at one refresh per second.
pub fn format_relative_time(now: DateTime<Utc>, stamp: DateTime<Utc>) -> String {
    let diff = now - stamp;
    if diff < Duration::zero() {
        return format!("+{}", format_past(-diff));
    }
    format_past(diff)
}

/// Formats a duration as the coarsest whole unit that fits.
fn format_past(diff: Duration) -> String {
    if diff < Duration::minutes(1) {
        return format!("{}s", diff.num_seconds());
    }
    if diff < Duration::hours(1) {
        return format!("{}m", diff.num_minutes());
    }
    if diff < Duration::hours(24) {
        return format!("{}h", diff.num_hours());
    }
    format!("{}d", diff.num_hours() / 24)
}

#[cfg(test)]
mod tests;
