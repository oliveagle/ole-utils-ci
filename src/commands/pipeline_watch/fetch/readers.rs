// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The per-project readers of one fetch round.
//!
//! Each reader owns one GitLab endpoint and one cache key, so the fetch
//! loop itself stays a readable list of calls.

use super::Collector;
use crate::commands::pipeline_watch::cache::{self, Cache, Value};
use crate::commands::pipeline_watch::client::{
    BranchHead, Client, Job, LatestReleaseInfo, Pipeline, Project,
};
use crate::commands::pipeline_watch::rows::release_branch_name;

/// Reads the pipelines of one project and the jobs of each of them.
pub fn read_pipelines(
    client: &Client,
    cache: &mut Cache,
    path: &str,
    per_repo: i64,
    collector: &mut Collector,
) {
    let pipelines = match cached_pipelines(client, cache, path, per_repo) {
        Ok(pipelines) => pipelines,
        Err(error) => {
            collector.errors.push(format!("{path} pipelines: {error}"));
            return;
        }
    };
    collector
        .pipelines
        .insert(path.to_string(), pipelines.clone());
    for pipeline in &pipelines {
        match cached_job_counts(client, cache, path, pipeline.id) {
            Ok((total, failed)) => {
                collector.jobs.insert(pipeline.id, total);
                collector.failed_jobs.insert(pipeline.id, failed);
            }
            Err(error) => collector
                .errors
                .push(format!("{path} #{} jobs: {error}", pipeline.id)),
        }
    }
}

/// Reads the open merge-request summary of one project.
pub fn read_merge_requests(
    client: &Client,
    cache: &mut Cache,
    path: &str,
    collector: &mut Collector,
) {
    match cached_merge_requests(client, cache, path) {
        Ok((count, status)) => {
            collector.mr_counts.insert(path.to_string(), count);
            collector.mr_pipelines.insert(path.to_string(), status);
        }
        Err(error) => collector.errors.push(format!("{path} MRs: {error}")),
    }
}

/// Reads the one-day commit counter of one project.
pub fn read_commits(client: &Client, cache: &mut Cache, path: &str, collector: &mut Collector) {
    match cached_commits(client, cache, path) {
        Ok(count) => {
            collector.commits.insert(path.to_string(), count);
        }
        Err(error) => collector.errors.push(format!("{path} commits: {error}")),
    }
}

/// Reads the newest release tag of one project.
pub fn read_tag(client: &Client, cache: &mut Cache, path: &str, collector: &mut Collector) {
    match cached_latest_tag(client, cache, path) {
        Ok(tag) => {
            if !tag.tag_name.is_empty() {
                collector.tags.insert(path.to_string(), tag);
            }
        }
        Err(error) => collector.errors.push(format!("{path} tags: {error}")),
    }
}

/// Reads the branch heads the ALIGN verdict needs.
///
/// Two heads per project: the default branch, and the strict-model release
/// branch when the newest tag names one.
pub fn read_branches(
    client: &Client,
    cache: &mut Cache,
    projects: &[Project],
    path: &str,
    collector: &mut Collector,
) {
    let default_branch = default_branch_for(projects, path);
    collector
        .default_branches
        .insert(path.to_string(), default_branch.clone());
    match cached_branch_head(client, cache, path, &default_branch) {
        Ok(head) => {
            if head.exists {
                collector
                    .branches
                    .insert(branch_key(path, &default_branch), head);
            }
        }
        Err(error) => collector
            .errors
            .push(format!("{path} branch {default_branch}: {error}")),
    }
    let release = release_branch_for(collector, path);
    if release.is_empty() {
        return;
    }
    match cached_branch_head(client, cache, path, &release) {
        Ok(head) => {
            if head.exists {
                collector.branches.insert(branch_key(path, &release), head);
            }
        }
        Err(error) => collector
            .errors
            .push(format!("{path} branch {release}: {error}")),
    }
}

/// The strict-model release branch of one project, if its newest tag names
/// one.
fn release_branch_for(collector: &Collector, path: &str) -> String {
    match collector.tags.get(path) {
        Some(tag) => release_branch_name(&tag.tag_name),
        None => String::new(),
    }
}

/// The pipelines of one project, from the cache when they are fresh.
fn cached_pipelines(
    client: &Client,
    cache: &mut Cache,
    path: &str,
    per_repo: i64,
) -> Result<Vec<Pipeline>, String> {
    let key = cache::pipelines_key(path, per_repo);
    if let Some(Value::Pipelines(pipelines)) = cache.fresh(&key, cache::TTL_PIPELINES) {
        return Ok(pipelines.clone());
    }
    let pipelines = client.list_project_pipelines_with_fallback(path, per_repo as usize)?;
    cache.put(&key, Value::Pipelines(pipelines.clone()));
    Ok(pipelines)
}

/// The job counts of one pipeline, from the cache when they are fresh.
fn cached_job_counts(
    client: &Client,
    cache: &mut Cache,
    path: &str,
    pipeline_id: i64,
) -> Result<(i64, i64), String> {
    let key = cache::jobs_key(path, pipeline_id);
    if let Some(Value::Jobs(jobs)) = cache.fresh(&key, cache::TTL_JOBS) {
        return Ok(count_jobs(jobs));
    }
    let jobs = client.list_pipeline_jobs(path, pipeline_id)?;
    let counts = count_jobs(&jobs);
    cache.put(&key, Value::Jobs(jobs));
    Ok(counts)
}

/// The total and failed job counts of a job list.
fn count_jobs(jobs: &[Job]) -> (i64, i64) {
    let mut failed = 0;
    for job in jobs {
        if job.status.eq_ignore_ascii_case("failed") {
            failed += 1;
        }
    }
    (jobs.len() as i64, failed)
}

/// The open merge-request count and worst status, from the cache when fresh.
fn cached_merge_requests(
    client: &Client,
    cache: &mut Cache,
    path: &str,
) -> Result<(i64, String), String> {
    let count_key = cache::mr_count_key(path);
    let worst_key = cache::mr_worst_key(path);
    if let Some(Value::Count(count)) = cache.fresh(&count_key, cache::TTL_MR) {
        let worst = match cache.fresh(&worst_key, cache::TTL_MR) {
            Some(Value::Status(status)) => status.clone(),
            _ => String::new(),
        };
        return Ok((*count, worst));
    }
    let (count, worst) = client.summarize_project_mrs(path)?;
    cache.put(&count_key, Value::Count(count));
    cache.put(&worst_key, Value::Status(worst.clone()));
    Ok((count, worst))
}

/// The one-day commit counter, from the cache when it is fresh.
fn cached_commits(client: &Client, cache: &mut Cache, path: &str) -> Result<i64, String> {
    let key = cache::commits_key(path);
    if let Some(Value::Count(count)) = cache.fresh(&key, cache::TTL_COMMITS) {
        return Ok(*count);
    }
    let count = client.count_project_recent_commits(path, 24)?;
    cache.put(&key, Value::Count(count));
    Ok(count)
}

/// The newest release tag, with a silent fallback to the stale value.
///
/// A slightly out-of-date tag beats a dash that flickers on every failed
/// API call.
fn cached_latest_tag(
    client: &Client,
    cache: &mut Cache,
    path: &str,
) -> Result<LatestReleaseInfo, String> {
    let key = cache::latest_tag_key(path);
    if let Some(Value::Tag(tag)) = cache.fresh(&key, cache::TTL_LATEST_TAG) {
        return Ok(tag.clone());
    }
    match client.get_latest_release_tag(path) {
        Ok(tag) => {
            cache.put(&key, Value::Tag(tag.clone()));
            Ok(tag)
        }
        Err(error) => match cache.stale(&key) {
            Some(Value::Tag(tag)) => Ok(tag.clone()),
            _ => Err(error),
        },
    }
}

/// The HEAD of one branch, with a silent fallback to the stale value.
fn cached_branch_head(
    client: &Client,
    cache: &mut Cache,
    path: &str,
    branch: &str,
) -> Result<BranchHead, String> {
    let key = cache::branch_head_key(path, branch);
    if let Some(Value::Branch(head)) = cache.fresh(&key, cache::TTL_BRANCH_HEAD) {
        return Ok(head.clone());
    }
    match client.get_branch_head(path, branch) {
        Ok(head) => {
            cache.put(&key, Value::Branch(head.clone()));
            Ok(head)
        }
        Err(error) => match cache.stale(&key) {
            Some(Value::Branch(head)) => Ok(head.clone()),
            _ => Err(error),
        },
    }
}

/// The default branch of one project, or `main` when the metadata is
/// missing.
fn default_branch_for(projects: &[Project], path: &str) -> String {
    projects
        .iter()
        .find(|project| project.path_with_namespace == path)
        .map(|project| project.default_branch.clone())
        .filter(|branch| !branch.is_empty())
        .unwrap_or_else(|| "main".to_string())
}

/// The key of one branch HEAD.
fn branch_key(path: &str, branch: &str) -> String {
    format!("{path}\u{0}{branch}")
}
