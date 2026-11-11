// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The branch HEAD lookup behind the dashboard's ALIGN column.

use serde::Deserialize;

use super::{BranchHead, Client, CommitEntry};

impl Client {
    /// The HEAD commit of one branch.
    ///
    /// A missing branch is not an error: a fresh project simply has no
    /// release branch yet, and reporting that as a failure would fill the
    /// dashboard's error strip with red rows.
    pub fn get_branch_head(
        &self,
        project_path: &str,
        branch_name: &str,
    ) -> Result<BranchHead, String> {
        if project_path.trim().is_empty() {
            return Err("project path is empty".to_string());
        }
        if branch_name.trim().is_empty() {
            return Err("branch name is empty".to_string());
        }
        let url = format!(
            "{}/api/v4/projects/{}/repository/branches/{}",
            self.base_url(),
            super::path_escape(project_path),
            super::path_escape(branch_name)
        );
        let response = self
            .get(&url)
            .map_err(|error| format!("branch {branch_name} in {project_path}: {error}"))?;
        if response.status == 404 {
            return Ok(BranchHead {
                name: branch_name.to_string(),
                ..BranchHead::default()
            });
        }
        if response.status == 401 {
            return Err(format!(
                "branch {branch_name} in {project_path}: 401 Unauthorized"
            ));
        }
        if !(200..300).contains(&response.status) {
            return Err(self.failure(
                &format!("branch {branch_name} in {project_path}"),
                &response,
            ));
        }
        let branch: BranchEntry = serde_json::from_slice(&response.body)
            .map_err(|error| format!("branch {branch_name} in {project_path}: decode: {error}"))?;
        Ok(BranchHead {
            name: branch_name.to_string(),
            sha: branch.commit.id.trim().to_string(),
            exists: true,
            default: branch.default,
        })
    }
}

/// One branch, reduced to its commit.
#[derive(Debug, Clone, Default, Deserialize)]
struct BranchEntry {
    /// The branch name.
    #[serde(default)]
    #[allow(dead_code)]
    name: String,
    /// Whether the branch is the project default.
    #[serde(default)]
    default: bool,
    /// The commit the branch points at.
    #[serde(default)]
    commit: CommitEntry,
}
