// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `--wait` poll loop of `pipeline-run`.
//!
//! The loop owns a deadline of its own: the 30s trigger timeout bounds only
//! the POST that creates the pipeline and is deliberately never reused as
//! the parent of this loop, so a pipeline that runs longer than the trigger
//! timeout still gets polled to completion.

use std::time::{Duration, Instant};

use super::model::PipelineRunResponse;
use crate::gitlab;
use crate::httpd;

/// How long one wait-loop GET may take; Go puts a 10s context around it.
const GET_TIMEOUT: Duration = Duration::from_secs(10);

/// Extra budget on top of `--wait-timeout` for the loop's parent context,
/// so the final in-flight GET always fits inside it.
const WAIT_MARGIN: Duration = Duration::from_secs(30);

/// `waitForPipeline`: poll until the pipeline reaches a terminal state, the
/// `--wait-timeout` deadline expires, or the parent budget runs out.
///
/// A transport error or a non-2xx answer is transient and retried; a body
/// that does not decode is fatal, exactly like the Go original.
pub(super) fn wait_for_pipeline(
    base_url: &str,
    project: &str,
    pipeline_id: i64,
    token: &str,
    timeout: i64,
    poll_step: Duration,
) -> Result<(String, f64), String> {
    let start = Instant::now();
    let budget = Duration::from_secs(timeout.max(0) as u64);
    let deadline = start + budget;
    let parent_deadline = start + budget + WAIT_MARGIN;
    let mut next_tick = start + poll_step;
    let url = format!(
        "{base_url}/api/v4/projects/{}/pipelines/{pipeline_id}",
        gitlab::path_escape(project)
    );
    let headers = [("PRIVATE-TOKEN", token), ("Accept", "application/json")];
    loop {
        if Instant::now() > deadline {
            return Err(format!(
                "timeout after {timeout}s waiting for pipeline #{pipeline_id}"
            ));
        }
        if let Ok(response) = httpd::request("GET", &url, &headers, None, GET_TIMEOUT) {
            if (200..300).contains(&response.status) {
                let pipeline: PipelineRunResponse = serde_json::from_slice(&response.body)
                    .map_err(|error| format!("decode: {error}"))?;
                if is_terminal(&pipeline.status) {
                    return Ok((pipeline.status, start.elapsed().as_secs_f64()));
                }
            }
        }
        wait_tick(&mut next_tick, poll_step, parent_deadline)?;
    }
}

/// The terminal states the loop stops at. `manual` and `scheduled`
/// deliberately keep it polling.
fn is_terminal(status: &str) -> bool {
    matches!(status, "success" | "failed" | "canceled" | "skipped")
}

/// Sleeps until the next tick the way `time.Ticker` does: one tick may be
/// buffered, so a late receiver resyncs instead of catching up, and the
/// parent deadline always wins over the next tick.
fn wait_tick(
    next_tick: &mut Instant,
    poll_step: Duration,
    parent_deadline: Instant,
) -> Result<(), String> {
    let now = Instant::now();
    let target = (*next_tick).min(parent_deadline);
    if target > now {
        std::thread::sleep(target - now);
    }
    let now = Instant::now();
    if now >= parent_deadline {
        return Err("context deadline exceeded".to_string());
    }
    *next_tick += poll_step;
    if *next_tick <= now {
        *next_tick = now + poll_step;
    }
    Ok(())
}
