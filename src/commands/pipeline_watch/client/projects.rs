// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Project enumeration: the group listing and the single-project read.

use super::{path_escape, Client, Project, MAX_GROUP_ITEMS};

impl Client {
    /// Enumerates the projects of a top-level group.
    ///
    /// GitLab paginates; the `Link: rel="next"` header is followed until it
    /// runs out, capped at 200 projects so a runaway group cannot exhaust
    /// the token's rate limit.
    pub fn list_group_projects(&self, group_path: &str) -> Result<Vec<Project>, String> {
        if group_path.trim().is_empty() {
            return Err("group path is empty".to_string());
        }
        let mut next = format!(
            "{}/api/v4/groups/{}/projects?per_page=100&order_by=last_activity_at",
            self.base,
            path_escape(group_path)
        );
        let mut out: Vec<Project> = Vec::new();
        while !next.is_empty() && out.len() < MAX_GROUP_ITEMS {
            let response = self
                .get(&next)
                .map_err(|error| format!("list group {group_path}: {error}"))?;
            if !(200..300).contains(&response.status) {
                // Go names the URL, not the group, in the page error.
                return Err(format!(
                    "list group {group_path}: {}",
                    self.failure(&next, &response)
                ));
            }
            let page: Vec<Project> = serde_json::from_slice(&response.body)
                .map_err(|error| format!("list group {group_path}: decode: {error}"))?;
            let count = page.len();
            out.extend(page);
            next = next_link(response.header("link").unwrap_or_default());
            if count == 0 {
                break;
            }
        }
        Ok(out)
    }

    /// One project by path.
    pub fn get_project(&self, project_path: &str) -> Result<Project, String> {
        if project_path.trim().is_empty() {
            return Err("project path is empty".to_string());
        }
        let url = format!(
            "{}/api/v4/projects/{}",
            self.base,
            path_escape(project_path)
        );
        let response = self
            .get(&url)
            .map_err(|error| format!("get project {project_path}: {error}"))?;
        if response.status == 404 {
            return Err(format!(
                "get project {project_path}: 404 (check name & token scope)"
            ));
        }
        if !(200..300).contains(&response.status) {
            return Err(self.failure(&format!("get project {project_path}"), &response));
        }
        serde_json::from_slice(&response.body).map_err(|error| format!("decode: {error}"))
    }
}

/// The `rel="next"` URL of a `Link` header, or an empty string.
fn next_link(header: &str) -> String {
    for part in header.split(',') {
        let segment = part.trim();
        if !segment.contains("rel=\"next\"") {
            continue;
        }
        if let (Some(start), Some(end)) = (segment.find('<'), segment.rfind('>')) {
            if end > start {
                return segment[start + 1..end].to_string();
            }
        }
    }
    String::new()
}
