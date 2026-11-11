// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `repo-status` payload.
//!
//! Field names, order and `omitempty` behaviour are the Go original's,
//! because agents key off them. Go's `omitempty` is not serde's: it drops
//! an empty string, slice or number, but never a struct — which is why
//! `created_at` is always present and a missing one prints Go's zero
//! timestamp instead of disappearing.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::commands::pipeline_watch::client::{Job, LatestReleaseInfo, Pipeline, Project};
use crate::gojson;

/// The whole answer to one `repo-status` call.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RepoStatus {
    /// The project the query named.
    pub project: ProjectInfo,
    /// The newest pipeline, repeated at the top for one-line answers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<PipelineInfo>,
    /// The pipelines the caller asked for, newest first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pipelines: Vec<PipelineInfo>,
    /// The open merge requests, summarised or listed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mrs: Option<MrSummary>,
    /// How many commits landed in the last 24 hours.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commits: Option<CommitInfo>,
    /// The newest release tag, when the project has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_tag: Option<LatestTag>,
    /// Where and when the payload came from.
    pub meta: Meta,
}

/// The project identity.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProjectInfo {
    /// `namespace/project`.
    pub path: String,
    /// The numeric id.
    #[serde(skip_serializing_if = "is_zero")]
    pub id: i64,
    /// The display name.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// The branch new pushes land on.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub default_branch: String,
    /// The browsable URL.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub web_url: String,
}

/// One pipeline, with the dashboard's effective status.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PipelineInfo {
    /// The pipeline id.
    pub id: i64,
    /// The status the dashboard paints.
    pub status: String,
    /// GitLab's flat status, kept so the two can be compared.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub raw_status: String,
    /// The git ref.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub r#ref: String,
    /// The commit SHA.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sha: String,
    /// push / web / schedule / ...
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// When the pipeline was created.
    #[serde(serialize_with = "gojson::serialize_stamp")]
    pub created_at: Option<DateTime<Utc>>,
    /// When the pipeline was last touched.
    #[serde(serialize_with = "gojson::serialize_stamp")]
    pub updated_at: Option<DateTime<Utc>>,
    /// The wall-clock duration, in seconds.
    #[serde(
        skip_serializing_if = "skip_zero_float",
        serialize_with = "gojson::serialize_float"
    )]
    pub duration_seconds: f64,
    /// The browsable URL.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub web_url: String,
    /// How many jobs the pipeline had.
    #[serde(skip_serializing_if = "is_zero")]
    pub job_count: i64,
    /// The jobs, when the caller asked for them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub jobs: Vec<JobInfo>,
}

/// One job of a pipeline.
#[derive(Debug, Clone, Default, Serialize)]
pub struct JobInfo {
    /// The job id.
    pub id: i64,
    /// The job name.
    pub name: String,
    /// The stage the job belongs to.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stage: String,
    /// The job status.
    pub status: String,
    /// When the job was created.
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "gojson::serialize_stamp_opt"
    )]
    pub created_at: Option<DateTime<Utc>>,
    /// When the job started.
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "gojson::serialize_stamp_opt"
    )]
    pub started_at: Option<DateTime<Utc>>,
    /// When the job finished.
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "gojson::serialize_stamp_opt"
    )]
    pub finished_at: Option<DateTime<Utc>>,
    /// The raw job log, with `--trace`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub trace: String,
    /// Why the log could not be read.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub trace_err: String,
}

/// The open merge requests of a project.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MrSummary {
    /// How many are open.
    pub open_count: i64,
    /// The worst head-pipeline status across them.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub head_pipeline_status: String,
    /// The list itself, worst-first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub list: Vec<MrInfo>,
}

/// One open merge request.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MrInfo {
    /// The project-scoped id.
    pub iid: i64,
    /// The title.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// The branch being merged.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source_branch: String,
    /// The branch it targets.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub target_branch: String,
    /// The browsable URL.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub web_url: String,
    /// The head pipeline's id.
    #[serde(skip_serializing_if = "is_zero")]
    pub head_pipeline_id: i64,
    /// The head pipeline's status.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub head_pipeline_status: String,
    /// Who opened it.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub author: String,
}

/// The commit count of the last day.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CommitInfo {
    /// Commits pushed in the last 24 hours.
    pub last_1d: i64,
}

/// The newest release tag.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LatestTag {
    /// The tag name.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub tag_name: String,
    /// The commit it points at.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub commit_sha: String,
    /// When it was published.
    #[serde(serialize_with = "gojson::serialize_stamp")]
    pub released_at: Option<DateTime<Utc>>,
    /// `releases` or `tags` — which endpoint answered.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source: String,
}

/// Where the payload came from.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Meta {
    /// When it was fetched.
    pub fetched_at: String,
    /// Where the token came from.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub token_source: String,
    /// The GitLab instance.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub base_url: String,
    /// Everything that was demoted from an error.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl ProjectInfo {
    /// Copies the fields of a fetched project.
    pub fn from_project(project: &Project) -> ProjectInfo {
        ProjectInfo {
            path: project.path_with_namespace.clone(),
            id: project.id,
            name: project.name.clone(),
            default_branch: project.default_branch.clone(),
            web_url: project.web_url.clone(),
        }
    }
}

impl PipelineInfo {
    /// Shapes one fetched pipeline, without its jobs.
    pub fn from_pipeline(pipeline: &Pipeline) -> PipelineInfo {
        PipelineInfo {
            id: pipeline.id,
            status: pipeline.effective_status(),
            raw_status: pipeline.status.clone(),
            r#ref: pipeline.r#ref.clone(),
            sha: pipeline.sha.clone(),
            source: pipeline.source.clone(),
            created_at: pipeline.created_at,
            updated_at: pipeline.updated_at,
            duration_seconds: pipeline.duration_seconds(),
            web_url: pipeline.web_url.clone(),
            job_count: 0,
            jobs: Vec::new(),
        }
    }
}

impl JobInfo {
    /// Shapes one fetched job, without its trace.
    pub fn from_job(job: &Job) -> JobInfo {
        JobInfo {
            id: job.id,
            name: job.name.clone(),
            stage: job.stage.clone(),
            status: job.status.clone(),
            created_at: job.created_at,
            started_at: job.started_at,
            finished_at: job.finished_at,
            trace: String::new(),
            trace_err: String::new(),
        }
    }
}

impl LatestTag {
    /// Shapes the fetched release info.
    pub fn from_release(info: &LatestReleaseInfo) -> LatestTag {
        LatestTag {
            tag_name: info.tag_name.clone(),
            commit_sha: info.commit_sha.clone(),
            released_at: info.released_at,
            source: info.source.clone(),
        }
    }
}

/// `omitempty` for an integer.
fn is_zero(value: &i64) -> bool {
    *value == 0
}

/// `omitempty` for a float.
fn skip_zero_float(value: &f64) -> bool {
    *value == 0.0
}

#[cfg(test)]
mod tests;
