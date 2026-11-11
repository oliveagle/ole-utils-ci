// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn job(id: i64, status: &str) -> JobInfo {
    JobInfo {
        id,
        status: status.to_string(),
        ..JobInfo::default()
    }
}

fn statuses(jobs: &[JobInfo]) -> Vec<String> {
    jobs.iter().map(|j| j.status.clone()).collect()
}

#[test]
fn failures_rise_to_the_top() {
    let mut jobs = vec![
        job(1, "success"),
        job(2, "failed"),
        job(3, "running"),
        job(4, "skipped"),
    ];
    sort_jobs(&mut jobs);
    assert_eq!(statuses(&jobs), ["failed", "running", "success", "skipped"]);
}

#[test]
fn canceled_outranks_running_but_not_failed() {
    let mut jobs = vec![job(1, "running"), job(2, "canceled"), job(3, "failed")];
    sort_jobs(&mut jobs);
    assert_eq!(statuses(&jobs), ["failed", "canceled", "running"]);
}

#[test]
fn equal_severity_keeps_the_lower_id_first() {
    let mut jobs = vec![job(9, "failed"), job(3, "failed"), job(7, "failed")];
    sort_jobs(&mut jobs);
    let ids: Vec<i64> = jobs.iter().map(|j| j.id).collect();
    assert_eq!(ids, [3, 7, 9]);
}

#[test]
fn an_unknown_status_sorts_last() {
    let mut jobs = vec![job(1, "mystery"), job(2, "manual")];
    sort_jobs(&mut jobs);
    assert_eq!(statuses(&jobs), ["manual", "mystery"]);
}

#[test]
fn the_rank_function_is_case_and_space_insensitive() {
    // GitLab has been seen to send " FAILED " from a proxy; the sort must
    // still treat it as the worst status.
    assert_eq!(job_rank(" FAILED "), job_rank("failed"));
    assert!(job_rank("FAILED") < job_rank("success"));
}
