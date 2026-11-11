// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The newest release tag of a project, for the dashboard's LATEST column.

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{Client, CommitEntry, LatestReleaseInfo};

impl Client {
    /// The newest release tag of a project.
    ///
    /// `/releases` is the operator's version of truth; `/tags` is the
    /// fallback for repos that have not cut a Release object yet.
    pub fn get_latest_release_tag(&self, project_path: &str) -> Result<LatestReleaseInfo, String> {
        if let Ok(mut info) = self.first_release(project_path) {
            if !info.tag_name.is_empty() {
                info.source = "releases".to_string();
                return Ok(info);
            }
        }
        self.first_tag(project_path)
    }

    /// The newest entry of `/releases`.
    ///
    /// An empty tag name with no error means "no releases yet", which is
    /// what lets the caller fall back to `/tags`.
    fn first_release(&self, project_path: &str) -> Result<LatestReleaseInfo, String> {
        let url = format!(
            "{}/api/v4/projects/{}/releases?per_page=1&order_by=released_at&sort=desc",
            self.base_url(),
            super::path_escape(project_path)
        );
        let response = self
            .get(&url)
            .map_err(|error| format!("releases {project_path}: {error}"))?;
        if response.status == 401 {
            return Err(format!("releases {project_path}: 401 Unauthorized"));
        }
        if !(200..300).contains(&response.status) {
            return Err(self.failure(&format!("releases {project_path}"), &response));
        }
        let releases: Vec<ReleaseEntry> = serde_json::from_slice(&response.body)
            .map_err(|error| format!("releases {project_path}: decode: {error}"))?;
        Ok(match releases.first() {
            None => LatestReleaseInfo::default(),
            Some(release) => LatestReleaseInfo {
                tag_name: release.tag_name.trim().to_string(),
                commit_sha: release.commit.id.trim().to_string(),
                released_at: release.released_at,
                source: String::new(),
            },
        })
    }

    /// The newest entry of `/tags`.
    fn first_tag(&self, project_path: &str) -> Result<LatestReleaseInfo, String> {
        let url = format!(
            "{}/api/v4/projects/{}/repository/tags?per_page=1&order_by=updated&sort=desc",
            self.base_url(),
            super::path_escape(project_path)
        );
        let response = self
            .get(&url)
            .map_err(|error| format!("tags {project_path}: {error}"))?;
        if !(200..300).contains(&response.status) {
            return Err(self.failure(&format!("tags {project_path}"), &response));
        }
        let tags: Vec<TagEntry> = serde_json::from_slice(&response.body)
            .map_err(|error| format!("tags {project_path}: decode: {error}"))?;
        Ok(match tags.first() {
            None => LatestReleaseInfo::default(),
            Some(tag) => LatestReleaseInfo {
                tag_name: tag.name.trim().to_string(),
                commit_sha: tag.commit.id.trim().to_string(),
                released_at: None,
                source: String::new(),
            },
        })
    }
}

/// One release, reduced to the tag and its commit.
#[derive(Debug, Clone, Default, Deserialize)]
struct ReleaseEntry {
    /// The tag name.
    #[serde(default)]
    tag_name: String,
    /// When the release was published.
    #[serde(default)]
    released_at: Option<DateTime<Utc>>,
    /// The commit the release points at.
    #[serde(default)]
    commit: CommitEntry,
}

/// One tag, reduced to its name and commit.
#[derive(Debug, Clone, Default, Deserialize)]
struct TagEntry {
    /// The tag name.
    #[serde(default)]
    name: String,
    /// The commit the tag points at.
    #[serde(default)]
    commit: CommitEntry,
}
