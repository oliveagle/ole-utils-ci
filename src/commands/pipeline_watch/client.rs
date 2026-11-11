// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The GitLab v4 client the dashboard reads from.
//!
//! The Go original uses `net/http`; this crate has no HTTP dependency, so
//! every call goes through [`crate::httpd::request`], which shells out to
//! `curl` and parses the reply. The URL shapes, the pagination rules and
//! the error wording are the Go ones.

use std::time::Duration;

use chrono::Utc;
use serde::Deserialize;

use crate::gitlab::path_escape;
use crate::httpd;

/// How long one GitLab call may take.
const TIMEOUT: Duration = Duration::from_secs(15);

/// The number of projects one group listing may return.
const MAX_GROUP_ITEMS: usize = 200;

mod branch;
mod model;
mod mr;
mod open_mrs;
mod projects;
mod tags;
mod trace;

pub use model::*;
pub(crate) use open_mrs::*;

/// A thin REST client for one GitLab instance.
#[derive(Debug, Clone)]
pub struct Client {
    base: String,
    token: String,
}

impl Client {
    /// A client for `base`, authenticating with `token`.
    pub fn new(base: &str, token: &str) -> Client {
        Client {
            base: base.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }

    /// The instance root this client talks to.
    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// One authenticated GET, returning the raw body.
    fn get(&self, url: &str) -> Result<httpd::Response, String> {
        let mut headers: Vec<(&str, &str)> = vec![("Accept", "application/json")];
        if !self.token.is_empty() {
            headers.push(("PRIVATE-TOKEN", self.token.as_str()));
        }
        httpd::request("GET", url, &headers, None, TIMEOUT)
    }

    /// The status line and a truncated body, the way the Go original
    /// reports a failed call.
    fn failure(&self, prefix: &str, response: &httpd::Response) -> String {
        format!(
            "{prefix}: {}: {}",
            response.status_display(),
            truncate(&response.body_string(), 200)
        )
    }

    /// The most recent pipelines of one project.
    pub fn list_project_pipelines(
        &self,
        project_path: &str,
        count: usize,
    ) -> Result<Vec<Pipeline>, String> {
        let count = if count == 0 { 5 } else { count };
        if project_path.trim().is_empty() {
            return Err("project path is empty".to_string());
        }
        let url = format!(
            "{}/api/v4/projects/{}/pipelines?per_page={count}&page=1",
            self.base,
            path_escape(project_path)
        );
        let response = self
            .get(&url)
            .map_err(|error| format!("pipelines {project_path}: {error}"))?;
        if response.status == 401 {
            return Err(format!(
                "pipelines {project_path}: 401 Unauthorized (token invalid or expired)"
            ));
        }
        if response.status == 429 {
            return Err(format!("pipelines {project_path}: 429 rate limited"));
        }
        if !(200..300).contains(&response.status) {
            return Err(self.failure(&format!("pipelines {project_path}"), &response));
        }
        serde_json::from_slice(&response.body)
            .map_err(|error| format!("pipelines {project_path}: decode: {error}"))
    }

    /// The most recent pipelines, retrying with an encoded path on a 404.
    ///
    /// Some legacy namespaces need the double-encoded form; the Go
    /// original works around it the same way.
    pub fn list_project_pipelines_with_fallback(
        &self,
        project_path: &str,
        count: usize,
    ) -> Result<Vec<Pipeline>, String> {
        match self.list_project_pipelines(project_path, count) {
            Ok(pipelines) => Ok(pipelines),
            Err(error) if error.contains("404") => {
                self.list_project_pipelines(&path_escape(project_path), count)
            }
            Err(error) => Err(error),
        }
    }

    /// The jobs of one pipeline.
    pub fn list_pipeline_jobs(
        &self,
        project_path: &str,
        pipeline_id: i64,
    ) -> Result<Vec<Job>, String> {
        if pipeline_id <= 0 {
            return Err(format!("invalid pipeline id {pipeline_id}"));
        }
        let url = format!(
            "{}/api/v4/projects/{}/pipelines/{pipeline_id}/jobs?per_page=100",
            self.base,
            path_escape(project_path)
        );
        let response = self
            .get(&url)
            .map_err(|error| format!("jobs {project_path}#{pipeline_id}: {error}"))?;
        if response.status == 401 {
            return Err(format!(
                "jobs {project_path}#{pipeline_id}: 401 Unauthorized"
            ));
        }
        if !(200..300).contains(&response.status) {
            return Err(self.failure(&format!("jobs {project_path}#{pipeline_id}"), &response));
        }
        serde_json::from_slice(&response.body)
            .map_err(|error| format!("jobs {project_path}#{pipeline_id}: decode: {error}"))
    }
    /// The number of commits pushed in the last `hours`.
    ///
    /// GitLab's pagination header carries the real total; older instances
    /// do not send it, and the page length is used instead.
    pub fn count_project_recent_commits(
        &self,
        project_path: &str,
        hours: i64,
    ) -> Result<i64, String> {
        if project_path.trim().is_empty() {
            return Err("project path is empty".to_string());
        }
        let hours = if hours <= 0 { 24 } else { hours };
        let since = (Utc::now() - chrono::Duration::hours(hours)).to_rfc3339();
        let url = format!(
            "{}/api/v4/projects/{}/repository/commits?since={}&per_page=1&page=1",
            self.base,
            path_escape(project_path),
            query_escape(&since)
        );
        let response = self
            .get(&url)
            .map_err(|error| format!("count commits {project_path}: {error}"))?;
        if response.status == 401 {
            return Err(format!("count commits {project_path}: 401 Unauthorized"));
        }
        if !(200..300).contains(&response.status) {
            return Err(self.failure(&format!("count commits {project_path}"), &response));
        }
        if let Some(total) = response.header("x-total") {
            return total.parse::<i64>().map_err(|error| {
                format!("count commits {project_path}: bad X-Total {total:?}: {error}")
            });
        }
        let commits: Vec<serde_json::Value> = serde_json::from_slice(&response.body)
            .map_err(|error| format!("count commits {project_path}: decode: {error}"))?;
        Ok(commits.len() as i64)
    }
}

/// A commit reference, reduced to its id.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct CommitEntry {
    /// The commit SHA.
    #[serde(default)]
    id: String,
}

/// Truncates a body for an error message.
fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let head: String = text.chars().take(limit).collect();
    format!("{head}...(truncated)")
}

/// Percent-encodes a query value, the way `url.QueryEscape` does.
fn query_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
