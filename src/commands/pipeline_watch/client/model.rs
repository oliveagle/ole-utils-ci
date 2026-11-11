// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The JSON shapes the dashboard reads out of GitLab.
//!
//! The structs mirror the payloads the Go original decodes. Fields the
//! dashboard does not read yet are kept, so the shape stays a faithful
//! copy of GitLab's answer and a later reader can pick them up.

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// A minimal subset of GitLab's `/projects` JSON.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Project {
    /// The numeric project id.
    #[serde(default)]
    #[allow(dead_code)]
    pub id: i64,
    /// `namespace/project`.
    #[serde(default)]
    pub path_with_namespace: String,
    /// The browsable URL of the project.
    #[serde(default)]
    #[allow(dead_code)]
    pub web_url: String,
    /// The project's default branch, usually `main` or `master`.
    #[serde(default)]
    pub default_branch: String,
    /// The bare project name.
    #[serde(default)]
    #[allow(dead_code)]
    pub name: String,
    /// GitLab's read-only mirror flag.
    #[serde(default)]
    pub archived: bool,
}

/// GitLab's rich status object. Only the `group` field matters: it is what
/// the web UI uses to tell "success" from "success-with-warnings".
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DetailedStatus {
    /// The canonical bucket: success, running, failed, ...
    #[serde(default)]
    pub group: String,
    /// The human label.
    #[serde(default)]
    #[allow(dead_code)]
    pub label: String,
    /// The icon name.
    #[serde(default)]
    #[allow(dead_code)]
    pub icon: String,
}

/// The minimal author JSON the dashboard renders.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct User {
    /// The login name.
    #[serde(default)]
    #[allow(dead_code)]
    pub username: String,
    /// The display name.
    #[serde(default)]
    #[allow(dead_code)]
    pub name: String,
}

/// One pipeline of a project.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Pipeline {
    /// The pipeline id.
    #[serde(default)]
    pub id: i64,
    /// The owning project id.
    #[serde(default)]
    #[allow(dead_code)]
    pub project_id: i64,
    /// The git ref the pipeline ran on.
    #[serde(default)]
    pub r#ref: String,
    /// The commit SHA.
    #[serde(default)]
    #[allow(dead_code)]
    pub sha: String,
    /// running / pending / success / failed / canceled / skipped / manual.
    #[serde(default)]
    pub status: String,
    /// push / web / trigger / schedule / ...
    #[serde(default)]
    pub source: String,
    /// When the pipeline was created.
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    /// When the pipeline was last touched.
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
    /// When the pipeline started running.
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    /// When the pipeline finished.
    #[serde(default)]
    pub finished_at: Option<DateTime<Utc>>,
    /// The authoritative duration, in seconds.
    #[serde(default)]
    pub duration: f64,
    /// Who triggered it.
    #[serde(default)]
    #[allow(dead_code)]
    pub user: Option<User>,
    /// The browsable URL of the pipeline.
    #[serde(default)]
    pub web_url: String,
    /// GitLab's rich status, when the instance sends it.
    #[serde(default)]
    pub detailed_status: Option<DetailedStatus>,
}

impl Pipeline {
    /// The status the dashboard paints.
    ///
    /// GitLab reports "success-with-warnings" in the detailed status while
    /// the flat field still says "success"; the yellow `!` the web UI shows
    /// is exactly that case, so the flat field alone is a lie.
    pub fn effective_status(&self) -> String {
        if let Some(detailed) = &self.detailed_status {
            let group = detailed.group.trim().to_lowercase();
            if group == "success-with-warnings" || group == "success_with_warnings" {
                return "warning".to_string();
            }
            if !group.is_empty() {
                return group;
            }
        }
        self.status.clone()
    }

    /// The best-effort wall-clock duration, in seconds.
    ///
    /// GitLab's list endpoint omits `duration` and the start/stop stamps, so
    /// the fallback is `updated_at - created_at`, which overstates the
    /// runtime by the queueing delay.
    pub fn duration_seconds(&self) -> f64 {
        if self.duration > 0.0 {
            return self.duration;
        }
        if let (Some(started), Some(finished)) = (self.started_at, self.finished_at) {
            if finished > started {
                return (finished - started).num_milliseconds() as f64 / 1000.0;
            }
        }
        if let (Some(created), Some(updated)) = (self.created_at, self.updated_at) {
            if updated > created {
                return (updated - created).num_milliseconds() as f64 / 1000.0;
            }
        }
        0.0
    }
}

/// One job of a pipeline.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Job {
    /// The job id.
    #[serde(default)]
    #[allow(dead_code)]
    pub id: i64,
    /// The job status.
    #[serde(default)]
    pub status: String,
    /// The job name.
    #[serde(default)]
    #[allow(dead_code)]
    pub name: String,
    /// The pipeline stage.
    #[serde(default)]
    #[allow(dead_code)]
    pub stage: String,
    /// When the job was created.
    #[serde(default)]
    #[allow(dead_code)]
    pub created_at: Option<DateTime<Utc>>,
    /// When the job started.
    #[serde(default)]
    #[allow(dead_code)]
    pub started_at: Option<DateTime<Utc>>,
    /// When the job finished.
    #[serde(default)]
    #[allow(dead_code)]
    pub finished_at: Option<DateTime<Utc>>,
}

/// The subset of a branch payload the dashboard reads.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BranchHead {
    /// The branch name.
    #[serde(default)]
    #[allow(dead_code)]
    pub name: String,
    /// The branch HEAD commit.
    #[serde(default)]
    pub sha: String,
    /// False when the branch genuinely does not exist.
    #[serde(default)]
    pub exists: bool,
    /// Whether the branch is the project default.
    #[serde(default)]
    #[allow(dead_code)]
    pub default: bool,
}

/// The newest release tag of a project.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LatestReleaseInfo {
    /// The tag name, e.g. `v0.3.3`.
    #[serde(default)]
    pub tag_name: String,
    /// The commit the tag points at.
    #[serde(default)]
    pub commit_sha: String,
    /// When the release was published.
    #[serde(default)]
    pub released_at: Option<DateTime<Utc>>,
    /// `releases` or `tags` — which endpoint answered.
    #[serde(default)]
    #[allow(dead_code)]
    pub source: String,
}
