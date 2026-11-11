// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The raw job log, which is what `repo-status --trace` embeds.

use super::Client;

impl Client {
    /// The raw trace of one job.
    ///
    /// A 404 means the trace does not exist yet (the job is still pending) or
    /// has expired, and is answered with an empty log rather than an error —
    /// the Go original treats "no output yet" as data, not as a failure.
    pub fn get_job_trace(&self, project_path: &str, job_id: i64) -> Result<String, String> {
        if job_id <= 0 {
            return Err(format!("invalid job id {job_id}"));
        }
        let url = format!(
            "{}/api/v4/projects/{}/jobs/{job_id}/trace",
            self.base_url(),
            super::path_escape(project_path)
        );
        let response = self
            .get(&url)
            .map_err(|error| format!("trace {project_path}#{job_id}: {error}"))?;
        if response.status == 401 {
            return Err(format!("trace {project_path}#{job_id}: 401 Unauthorized"));
        }
        if response.status == 404 {
            return Ok(String::new());
        }
        if !(200..300).contains(&response.status) {
            return Err(self.failure(&format!("trace {project_path}#{job_id}"), &response));
        }
        Ok(response.body_string())
    }
}
