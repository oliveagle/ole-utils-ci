// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The open-merge-request list behind `repo-status --mrs`.
//!
//! The dashboard only needs a count and a worst-status roll-up, which
//! [`super::mr`] serves in one call. `repo-status` needs the rows themselves,
//! so this is the wider query. Both live in the client so the HTTP plumbing
//! stays in one place.

use serde::Deserialize;

use super::{Client, DetailedStatus};

/// One open merge request, as GitLab lists it.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OpenMr {
    /// The project-scoped id.
    #[serde(default)]
    pub iid: i64,
    /// The title.
    #[serde(default)]
    pub title: String,
    /// The branch being merged.
    #[serde(default)]
    pub source_branch: String,
    /// The branch it targets.
    #[serde(default)]
    pub target_branch: String,
    /// The browsable URL.
    #[serde(default)]
    pub web_url: String,
    /// The head pipeline, when the merge request has one.
    #[serde(default)]
    pub head_pipeline: Option<MrHeadPipeline>,
    /// Who opened it.
    #[serde(default)]
    pub author: Option<MrAuthor>,
}

/// The head pipeline of a merge request, reduced to what the list shows.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MrHeadPipeline {
    /// The pipeline id.
    #[serde(default)]
    pub id: i64,
    /// GitLab's flat status.
    #[serde(default)]
    pub status: String,
    /// The rich status, which is where "success-with-warnings" hides.
    #[serde(default)]
    pub detailed_status: Option<DetailedStatus>,
}

/// The author of a merge request.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MrAuthor {
    /// The login name.
    #[serde(default)]
    pub username: String,
}

impl MrHeadPipeline {
    /// The status the list shows.
    ///
    /// The rich status wins when it carries a bucket of its own, and
    /// `success-with-warnings` is reported as `warning` so the list agrees
    /// with the dashboard's MR-dot and with the roll-up count.
    pub fn effective_status(&self) -> String {
        let flat = self.status.trim().to_lowercase();
        let Some(detailed) = &self.detailed_status else {
            return flat;
        };
        let group = detailed.group.trim().to_lowercase();
        if group == "success-with-warnings" || group == "success_with_warnings" {
            return "warning".to_string();
        }
        if group.is_empty() {
            return flat;
        }
        group
    }
}

impl Client {
    /// The open merge requests of one project, in GitLab's order.
    pub fn list_open_mrs(&self, project_path: &str) -> Result<Vec<OpenMr>, String> {
        if project_path.trim().is_empty() {
            return Err("project path is empty".to_string());
        }
        let url = format!(
            "{}/api/v4/projects/{}/merge_requests?state=opened&per_page=50&with_labels_details=false",
            self.base_url(),
            super::path_escape(project_path)
        );
        let response = self
            .get(&url)
            .map_err(|error| format!("open MRs: {error}"))?;
        if !(200..300).contains(&response.status) {
            return Err(self.failure("open MRs", &response));
        }
        serde_json::from_slice(&response.body).map_err(|error| format!("open MRs: decode: {error}"))
    }
}
