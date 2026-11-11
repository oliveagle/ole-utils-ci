// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The dashboard's in-memory TTL cache.
//!
//! The Go original keeps the cache purely in memory too: no hydrate on
//! start, no flush after a refresh. Writing the whole cache on every
//! entry update across all watched repos thrashed the filesystem for no
//! long-lived benefit — a restart is meant to re-hit the API.
//!
//! The TTL is per entry, so slow-changing data (the project list) can live
//! longer than fast-changing data (the pipeline list).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::client::{BranchHead, Job, LatestReleaseInfo, Pipeline, Project};

/// The project list: membership barely changes.
pub const TTL_PROJECTS: Duration = Duration::from_secs(3600);
/// The pipeline list: one refresh cadence.
pub const TTL_PIPELINES: Duration = Duration::from_secs(30);
/// The merge-request summary.
pub const TTL_MR: Duration = Duration::from_secs(60);
/// The one-day commit counter.
pub const TTL_COMMITS: Duration = Duration::from_secs(120);
/// The job list: jobs are immutable once the pipeline finishes.
pub const TTL_JOBS: Duration = Duration::from_secs(300);
/// The newest release tag.
pub const TTL_LATEST_TAG: Duration = Duration::from_secs(300);
/// The branch HEAD.
pub const TTL_BRANCH_HEAD: Duration = Duration::from_secs(300);

/// The key of the project list.
pub const KEY_PROJECTS: &str = "projects";

/// The key of one project's pipeline list.
pub fn pipelines_key(project: &str, count: i64) -> String {
    format!("pipelines:{project}:{count}")
}

/// The key of one project's open merge-request count.
pub fn mr_count_key(project: &str) -> String {
    format!("mr_count:{project}")
}

/// The key of one project's worst merge-request pipeline status.
pub fn mr_worst_key(project: &str) -> String {
    format!("mr_worst_pipe:{project}")
}

/// The key of one project's one-day commit count.
pub fn commits_key(project: &str) -> String {
    format!("commits_1d:{project}")
}

/// The key of one pipeline's job list.
pub fn jobs_key(project: &str, pipeline_id: i64) -> String {
    format!("jobs:{project}:{pipeline_id}")
}

/// The key of one project's newest release tag.
pub fn latest_tag_key(project: &str) -> String {
    format!("latest_tag:{project}")
}

/// The key of one branch HEAD.
pub fn branch_head_key(project: &str, branch: &str) -> String {
    format!("branch_head:{project}:{branch}")
}

/// One cached value.
#[derive(Debug, Clone)]
pub enum Value {
    /// The project list.
    Projects(Vec<Project>),
    /// One project's pipelines.
    Pipelines(Vec<Pipeline>),
    /// One pipeline's jobs.
    Jobs(Vec<Job>),
    /// A counted number.
    Count(i64),
    /// A status word.
    Status(String),
    /// One project's newest release tag.
    Tag(LatestReleaseInfo),
    /// One branch's HEAD.
    Branch(BranchHead),
}

/// The cache itself: a map from key to value plus its stamp.
#[derive(Debug, Default)]
pub struct Cache {
    /// The entries, newest stamp first.
    entries: HashMap<String, (Instant, Value)>,
}

impl Cache {
    /// An empty cache.
    pub fn new() -> Cache {
        Cache::default()
    }

    /// A fresh value, if the entry exists and is still inside its TTL.
    pub fn fresh(&self, key: &str, ttl: Duration) -> Option<&Value> {
        let (stored, value) = self.entries.get(key)?;
        if stored.elapsed() > ttl {
            return None;
        }
        Some(value)
    }

    /// A value of any age, if the entry exists.
    ///
    /// The tag and branch-HEAD lookups use this as a silent fallback: a
    /// slightly out-of-date value beats a flickering dash.
    pub fn stale(&self, key: &str) -> Option<&Value> {
        self.entries.get(key).map(|(_, value)| value)
    }

    /// Stores a value and stamps it with the current time.
    pub fn put(&mut self, key: &str, value: Value) {
        self.entries
            .insert(key.to_string(), (Instant::now(), value));
    }
}
