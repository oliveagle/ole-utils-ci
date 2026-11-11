// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The live query behind `repo-status`.
//!
//! Every call hits GitLab; there is no cache, because an agent has to reason
//! off the state of the world rather than off a rendering of it. A failed
//! sub-query is demoted to `meta.warnings` so the primary answer — the
//! project and its pipelines — still reaches the caller.

use crate::commands::pipeline_watch::client::{Client, Pipeline};

use super::flags::Flags;
use super::model::{
    CommitInfo, JobInfo, LatestTag, MrSummary, PipelineInfo, ProjectInfo, RepoStatus,
};
use super::mrs;

/// How far back `--pipeline` scans for the id it was given.
const SINGLE_PIPELINE_SCAN: usize = 100;
/// The window the commit count covers, in hours.
const COMMIT_WINDOW_HOURS: i64 = 24;

/// Runs the aggregated query.
pub(super) fn fetch_repo_status(
    client: &Client,
    project_path: &str,
    flags: &Flags,
) -> Result<RepoStatus, String> {
    let project = client
        .get_project(project_path)
        .map_err(|error| format!("resolve project: {error}"))?;
    let mut out = RepoStatus {
        project: ProjectInfo::from_project(&project),
        ..RepoStatus::default()
    };

    for (index, pipeline) in collect_pipelines(client, project_path, flags)?
        .iter()
        .enumerate()
    {
        let mut info = PipelineInfo::from_pipeline(pipeline);
        attach_jobs(
            client,
            project_path,
            flags,
            &mut info,
            &mut out.meta.warnings,
        );
        if index == 0 {
            out.head = Some(info.clone());
        }
        out.pipelines.push(info);
    }

    out.mrs = fetch_mrs(client, project_path, flags, &mut out.meta.warnings);
    out.latest_tag = fetch_latest_tag(client, project_path, &mut out.meta.warnings);
    out.commits = fetch_commits(client, project_path, &mut out.meta.warnings);
    Ok(out)
}

/// The pipelines the caller asked for, newest first.
fn collect_pipelines(
    client: &Client,
    project_path: &str,
    flags: &Flags,
) -> Result<Vec<Pipeline>, String> {
    if flags.pipeline_id > 0 {
        let id = flags.pipeline_id;
        let pipeline = fetch_single_pipeline(client, project_path, id)
            .map_err(|error| format!("pipeline #{id}: {error}"))?;
        return Ok(vec![pipeline]);
    }
    let mut pipelines = client
        .list_project_pipelines_with_fallback(project_path, flags.pipelines as usize)
        .map_err(|error| format!("list pipelines: {error}"))?;
    // GitLab may return more than `per_page` when the server caps are
    // relaxed; the caller's window still wins.
    pipelines.truncate(flags.pipelines as usize);
    Ok(pipelines)
}

/// One pipeline, found by scanning the recent list.
///
/// GitLab's list endpoint already carries everything this command prints, so
/// the id is looked up rather than fetched from a detail endpoint.
fn fetch_single_pipeline(
    client: &Client,
    project_path: &str,
    pipeline_id: i64,
) -> Result<Pipeline, String> {
    let pipelines = client
        .list_project_pipelines_with_fallback(project_path, SINGLE_PIPELINE_SCAN)
        .map_err(|error| error.to_string())?;
    pipelines
        .into_iter()
        .find(|pipeline| pipeline.id == pipeline_id)
        .ok_or_else(|| {
            format!(
                "pipeline #{pipeline_id} not found in the last {SINGLE_PIPELINE_SCAN} pipelines of {project_path}"
            )
        })
}

/// Adds the job list, and the logs when asked, to one pipeline.
fn attach_jobs(
    client: &Client,
    project_path: &str,
    flags: &Flags,
    info: &mut PipelineInfo,
    warnings: &mut Vec<String>,
) {
    if !flags.include_jobs {
        return;
    }
    let jobs = match client.list_pipeline_jobs(project_path, info.id) {
        Ok(jobs) => jobs,
        Err(error) => {
            warnings.push(format!("pipeline #{} jobs: {error}", info.id));
            return;
        }
    };
    info.job_count = jobs.len() as i64;
    info.jobs = jobs
        .iter()
        .map(|job| {
            let mut shaped = JobInfo::from_job(job);
            if flags.include_trace {
                match client.get_job_trace(project_path, job.id) {
                    Ok(trace) => shaped.trace = trace,
                    Err(error) => shaped.trace_err = error,
                }
            }
            shaped
        })
        .collect();
    sort_jobs(&mut info.jobs);
}

/// Reorders jobs so the ones worth reading come first.
///
/// The order matches the log viewer's, so an agent and an operator look at
/// the same job first.
pub(super) fn sort_jobs(jobs: &mut [JobInfo]) {
    jobs.sort_by(|left, right| {
        job_rank(&left.status)
            .cmp(&job_rank(&right.status))
            .then(left.id.cmp(&right.id))
    });
}

/// The alarm level of a job status; lower is worse.
fn job_rank(status: &str) -> u8 {
    match status.trim().to_lowercase().as_str() {
        "failed" => 0,
        "canceled" | "cancelled" => 1,
        "running" => 2,
        "pending" | "created" => 3,
        "success" => 4,
        "skipped" | "manual" => 5,
        _ => 6,
    }
}

/// The merge-request roll-up, listed or summarised.
fn fetch_mrs(
    client: &Client,
    project_path: &str,
    flags: &Flags,
    warnings: &mut Vec<String>,
) -> Option<MrSummary> {
    if flags.mrs {
        return match mrs::fetch_open_mrs(client, project_path) {
            Ok(summary) => Some(summary),
            Err(error) => {
                warnings.push(format!("mrs: {error}"));
                None
            }
        };
    }
    // The cheap answer: a count and the worst head-pipeline status.
    match client.summarize_project_mrs(project_path) {
        Ok((count, status)) => Some(MrSummary {
            open_count: count,
            head_pipeline_status: status,
            list: Vec::new(),
        }),
        Err(error) => {
            warnings.push(format!("mrs summary: {error}"));
            None
        }
    }
}

/// The newest release tag, or nothing when the project has never been cut.
fn fetch_latest_tag(
    client: &Client,
    project_path: &str,
    warnings: &mut Vec<String>,
) -> Option<LatestTag> {
    match client.get_latest_release_tag(project_path) {
        Ok(info) if !info.tag_name.is_empty() => Some(LatestTag::from_release(&info)),
        Ok(_) => None,
        Err(error) => {
            warnings.push(format!("latest_tag: {error}"));
            None
        }
    }
}

/// How many commits landed in the last day.
fn fetch_commits(
    client: &Client,
    project_path: &str,
    warnings: &mut Vec<String>,
) -> Option<CommitInfo> {
    match client.count_project_recent_commits(project_path, COMMIT_WINDOW_HOURS) {
        Ok(count) => Some(CommitInfo { last_1d: count }),
        Err(error) => {
            warnings.push(format!("commits: {error}"));
            None
        }
    }
}

#[cfg(test)]
mod tests;
