// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The three-stage fetch that fills the dashboard.
//!
//! Stage 1 enumerates the projects, stage 2 reads the pipelines, merge
//! requests, commit counters, tags and branch heads, and stage 3 reads the
//! jobs of every pipeline. Everything is sequential on purpose: a burst of
//! parallel calls is what gets a GitLab token rate-limited.
//!
//! A failing project never blanks the dashboard. Its error goes into the
//! error list and the rest of the rows still render.

mod readers;

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::gitlab::{WatchConfig, WatchRepo};

use super::cache::{self, Cache, Value};
use super::client::{BranchHead, Client, LatestReleaseInfo, Pipeline, Project};
use super::local;
use super::rows::{self, RepoRow, Sources};

/// The outcome of one fetch round.
pub struct FetchResult {
    /// The rows to render.
    pub rows: Vec<RepoRow>,
    /// The errors collected along the way.
    pub errors: Vec<String>,
}

/// The maps one fetch round fills.
pub struct Collector {
    /// The pipelines of every project, newest first.
    pub pipelines: HashMap<String, Vec<Pipeline>>,
    /// The job count of every pipeline.
    pub jobs: HashMap<i64, i64>,
    /// The failed-job count of every pipeline.
    pub failed_jobs: HashMap<i64, i64>,
    /// The open merge-request count of every project.
    pub mr_counts: HashMap<String, i64>,
    /// The worst merge-request pipeline status of every project.
    pub mr_pipelines: HashMap<String, String>,
    /// The one-day commit count of every project.
    pub commits: HashMap<String, i64>,
    /// The newest release tag of every project.
    pub tags: HashMap<String, LatestReleaseInfo>,
    /// The branch heads, keyed by project path plus branch name.
    pub branches: HashMap<String, BranchHead>,
    /// The default branch of every project.
    pub default_branches: HashMap<String, String>,
    /// The errors collected along the way.
    pub errors: Vec<String>,
}

impl Collector {
    /// An empty collector.
    fn new() -> Collector {
        Collector {
            pipelines: HashMap::new(),
            jobs: HashMap::new(),
            failed_jobs: HashMap::new(),
            mr_counts: HashMap::new(),
            mr_pipelines: HashMap::new(),
            commits: HashMap::new(),
            tags: HashMap::new(),
            branches: HashMap::new(),
            default_branches: HashMap::new(),
            errors: Vec::new(),
        }
    }
}

/// Fetches one round of dashboard data.
pub fn fetch_dashboard(
    client: &Client,
    config: &WatchConfig,
    cache: &mut Cache,
    now: DateTime<Utc>,
) -> FetchResult {
    let per_repo = if config.per_repo <= 0 {
        7
    } else {
        config.per_repo
    };
    let (projects, resolve_errors) = resolve_projects(client, &config.repos, cache);
    let mut collector = Collector::new();
    collector.errors.extend(resolve_errors);

    for path in project_paths(&projects) {
        // An archived mirror has no pipelines, merge requests or releases
        // worth polling, so it costs zero API calls. The row still shows up.
        if archived_for(&projects, &path) {
            continue;
        }
        readers::read_pipelines(client, cache, &path, per_repo, &mut collector);
        readers::read_merge_requests(client, cache, &path, &mut collector);
        readers::read_commits(client, cache, &path, &mut collector);
        readers::read_tag(client, cache, &path, &mut collector);
        readers::read_branches(client, cache, &projects, &path, &mut collector);
    }

    let sources = Sources {
        pipelines: &collector.pipelines,
        jobs: &collector.jobs,
        failed_jobs: &collector.failed_jobs,
        mr_counts: &collector.mr_counts,
        mr_pipelines: &collector.mr_pipelines,
        commits: &collector.commits,
    };
    let mut rows = rows::build_rows(&projects, &sources, per_repo as usize, now);
    rows::annotate_latest_tags(&mut rows, &collector.tags, now);
    rows::annotate_branch_align(&mut rows, &collector.branches, &collector.default_branches);
    local::annotate_local_divergence(&mut rows, &config.local_root);
    FetchResult {
        rows,
        errors: collector.errors,
    }
}

/// The project paths, in the sorted order the rows expect.
fn project_paths(projects: &[Project]) -> Vec<String> {
    projects
        .iter()
        .map(|project| project.path_with_namespace.clone())
        .collect()
}

/// Whether a project is a read-only mirror.
fn archived_for(projects: &[Project], path: &str) -> bool {
    projects
        .iter()
        .find(|project| project.path_with_namespace == path)
        .map(|project| project.archived)
        .unwrap_or(false)
}

/// Expands the configured groups and projects into the project list.
fn resolve_projects(
    client: &Client,
    repos: &[WatchRepo],
    cache: &mut Cache,
) -> (Vec<Project>, Vec<String>) {
    if let Some(Value::Projects(projects)) = cache.fresh(cache::KEY_PROJECTS, cache::TTL_PROJECTS) {
        return (projects.clone(), Vec::new());
    }
    let mut found: Vec<Project> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for repo in repos {
        match resolve_one(client, repo) {
            Ok(projects) => found.extend(projects),
            Err(error) => errors.push(error),
        }
    }
    found.sort_by(|left, right| left.path_with_namespace.cmp(&right.path_with_namespace));
    found.dedup_by(|left, right| left.path_with_namespace == right.path_with_namespace);
    if !found.is_empty() {
        cache.put(cache::KEY_PROJECTS, Value::Projects(found.clone()));
    }
    (found, errors)
}

/// Resolves one configured repo entry.
fn resolve_one(client: &Client, repo: &WatchRepo) -> Result<Vec<Project>, String> {
    if !repo.group.is_empty() {
        return client
            .list_group_projects(&repo.group)
            .map_err(|error| format!("group \"{}\": {error}", repo.group));
    }
    if !repo.project.is_empty() {
        return client
            .get_project(&repo.project)
            .map(|project| vec![project])
            .map_err(|error| format!("project \"{}\": {error}", repo.project));
    }
    Err(format!(
        "repo entry missing group/project: group={} project={}",
        repo.group, repo.project
    ))
}

#[cfg(test)]
mod tests;
