// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

use crate::commands::pipeline_watch::client::{MrAuthor, MrHeadPipeline};

use super::*;

fn mr(iid: i64, status: &str) -> MrInfo {
    MrInfo {
        iid,
        head_pipeline_status: status.to_string(),
        ..MrInfo::default()
    }
}

#[test]
fn the_worst_head_pipeline_comes_first() {
    let mut list = vec![
        mr(1, "success"),
        mr(2, "failed"),
        mr(3, ""),
        mr(4, "running"),
    ];
    sort_by_priority(&mut list);
    let iids: Vec<i64> = list.iter().map(|m| m.iid).collect();
    assert_eq!(iids, [2, 4, 1, 3]);
}

#[test]
fn equal_severity_puts_the_newest_first() {
    let mut list = vec![mr(4, "failed"), mr(9, "failed"), mr(6, "failed")];
    sort_by_priority(&mut list);
    let iids: Vec<i64> = list.iter().map(|m| m.iid).collect();
    assert_eq!(iids, [9, 6, 4]);
}

#[test]
fn a_warning_sits_with_the_in_progress_states() {
    assert_eq!(severity_rank("warning"), severity_rank("running"));
    assert!(severity_rank("canceled") > severity_rank("warning"));
    assert!(severity_rank("failed") > severity_rank("canceled"));
}

#[test]
fn an_unknown_status_is_the_least_urgent() {
    assert_eq!(severity_rank("something-new"), 0);
    assert!(severity_rank("success") > severity_rank("something-new"));
}

#[test]
fn success_with_warnings_is_reported_as_warning() {
    let pipeline = MrHeadPipeline {
        id: 7,
        status: "success".to_string(),
        detailed_status: Some(crate::commands::pipeline_watch::client::DetailedStatus {
            group: "success-with-warnings".to_string(),
            ..Default::default()
        }),
    };
    assert_eq!(pipeline.effective_status(), "warning");
}

#[test]
fn the_detailed_group_wins_over_the_flat_status() {
    let pipeline = MrHeadPipeline {
        id: 7,
        status: "running".to_string(),
        detailed_status: Some(crate::commands::pipeline_watch::client::DetailedStatus {
            group: "failed".to_string(),
            ..Default::default()
        }),
    };
    assert_eq!(pipeline.effective_status(), "failed");
}

#[test]
fn an_empty_group_falls_back_to_the_flat_status() {
    let pipeline = MrHeadPipeline {
        id: 7,
        status: "Manual".to_string(),
        detailed_status: Some(crate::commands::pipeline_watch::client::DetailedStatus::default()),
    };
    assert_eq!(pipeline.effective_status(), "manual");
}

#[test]
fn an_author_and_a_head_pipeline_are_optional() {
    let author = MrAuthor {
        username: "alice".to_string(),
    };
    assert_eq!(author.username, "alice");
    assert_eq!(MrHeadPipeline::default().effective_status(), "");
}
