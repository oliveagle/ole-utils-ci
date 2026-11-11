// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The merge-request half of `repo-status`.
//!
//! Two shapes exist because two callers want different things: the dashboard
//! wants a count and a worst-status roll-up, and `--mrs` wants the rows. The
//! rows are ordered worst-first so the agent reads the merge request that
//! needs attention before the one that is merely open.

use crate::commands::pipeline_watch::client::{Client, MrHeadPipeline, OpenMr};

use super::model::{MrInfo, MrSummary};

/// The open merge requests, with the roll-up the dashboard would show.
pub(super) fn fetch_open_mrs(client: &Client, project_path: &str) -> Result<MrSummary, String> {
    let raw = client.list_open_mrs(project_path)?;
    let (count, head_status) = client.summarize_project_mrs(project_path)?;
    let mut list: Vec<MrInfo> = raw.iter().map(shape).collect();
    sort_by_priority(&mut list);
    Ok(MrSummary {
        open_count: count,
        head_pipeline_status: head_status,
        list,
    })
}

/// Shapes one GitLab merge request into the list entry.
fn shape(entry: &OpenMr) -> MrInfo {
    MrInfo {
        iid: entry.iid,
        title: entry.title.clone(),
        source_branch: entry.source_branch.clone(),
        target_branch: entry.target_branch.clone(),
        web_url: entry.web_url.clone(),
        head_pipeline_id: entry.head_pipeline.as_ref().map_or(0, |p| p.id),
        head_pipeline_status: entry
            .head_pipeline
            .as_ref()
            .map(MrHeadPipeline::effective_status)
            .unwrap_or_default(),
        author: entry
            .author
            .as_ref()
            .map(|a| a.username.clone())
            .unwrap_or_default(),
    }
}

/// Reorders the list so the merge requests worth reading come first.
///
/// The bucket order mirrors the dashboard's MR-dot roll-up, and within one
/// bucket the newest merge request wins.
pub(super) fn sort_by_priority(list: &mut [MrInfo]) {
    list.sort_by(|left, right| {
        let rank = severity_rank(&right.head_pipeline_status)
            .cmp(&severity_rank(&left.head_pipeline_status));
        if rank.is_ne() {
            return rank;
        }
        right.iid.cmp(&left.iid)
    });
}

/// The alarm level of a head-pipeline status; higher is worse.
fn severity_rank(status: &str) -> u8 {
    match status.trim().to_lowercase().as_str() {
        "failed" => 4,
        "canceled" | "cancelled" => 3,
        "running" | "pending" | "created" => 2,
        // Green-with-warnings is on the radar but not a regression, so it
        // shares a bucket with the in-progress states, matching the MR-dot.
        "warning" | "success-with-warnings" | "success_with_warnings" => 2,
        "success" => 1,
        // No head pipeline, or a status GitLab added later.
        _ => 0,
    }
}

#[cfg(test)]
mod tests;
