// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The open-merge-request summary behind the dashboard's MR cell.

use serde::Deserialize;

use super::{Client, Pipeline};

impl Client {
    /// The open-MR count and the worst head-pipeline status across them.
    ///
    /// One call instead of the older `?per_page=1` count: the count still
    /// comes from the `X-Total` header, and the first page of merge
    /// requests carries the head pipeline the MR cell colours.
    pub fn summarize_project_mrs(&self, project_path: &str) -> Result<(i64, String), String> {
        if project_path.trim().is_empty() {
            return Err("project path is empty".to_string());
        }
        let url = format!(
            "{}/api/v4/projects/{}/merge_requests?state=opened&per_page=20&page=1",
            self.base_url(),
            super::path_escape(project_path)
        );
        let response = self
            .get(&url)
            .map_err(|error| format!("summarize MRs {project_path}: {error}"))?;
        if response.status == 401 {
            return Err(format!("summarize MRs {project_path}: 401 Unauthorized"));
        }
        if !(200..300).contains(&response.status) {
            return Err(self.failure(&format!("summarize MRs {project_path}"), &response));
        }
        let entries: Vec<MrEntry> = serde_json::from_slice(&response.body)
            .map_err(|error| format!("summarize MRs {project_path}: decode: {error}"))?;
        let mut total = entries.len() as i64;
        if let Some(header) = response.header("x-total") {
            if let Ok(parsed) = header.parse::<i64>() {
                total = parsed;
            }
        }
        let mut worst = String::new();
        for entry in &entries {
            if let Some(pipeline) = &entry.head_pipeline {
                worst = worse_mr_pipeline_status(&worst, &pipeline.effective_status());
            }
        }
        Ok((total, worst))
    }
}

/// One merge request, reduced to what the MR cell needs.
#[derive(Debug, Clone, Default, Deserialize)]
struct MrEntry {
    /// The merge request id.
    #[serde(default)]
    #[allow(dead_code)]
    id: i64,
    /// The head pipeline, when the merge request has one.
    #[serde(default)]
    head_pipeline: Option<Pipeline>,
}

/// The alarm level of an MR head-pipeline status; higher is worse.
fn mr_status_rank(status: &str) -> u8 {
    match status.trim().to_lowercase().as_str() {
        "failed" => 4,
        "canceled" | "cancelled" => 3,
        "running" | "pending" | "created" => 2,
        // A green-with-warnings merge request is worse than clean success
        // but better than "still running": on the radar, not a regression.
        "warning" => 2,
        "success" => 1,
        _ => 0,
    }
}

/// The worse of two MR head-pipeline statuses, in canonical lower case.
pub fn worse_mr_pipeline_status(current: &str, candidate: &str) -> String {
    if mr_status_rank(candidate) > mr_status_rank(current) {
        return candidate.trim().to_lowercase();
    }
    current.trim().to_lowercase()
}
