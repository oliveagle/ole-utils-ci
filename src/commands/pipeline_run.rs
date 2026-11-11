// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `pipeline-run` command: trigger a GitLab pipeline for a project and
//! report what happened.
//!
//! Rust port of the Go original's `internal/pipeline_run.go`. The command
//! owns a private flag set and therefore receives the raw argument list —
//! the shared global parser must never see these flags. Token resolution,
//! the base URL and the variable list all follow the Go fallback chain, and
//! every refusal travels up so `main` prints its own `Error:` line.

mod flags;
mod model;
mod usage;
mod wait;

use std::time::Duration;

use crate::gitlab;
use crate::httpd;

use flags::{parse_flags, RunFlags, DEFAULT_WAIT_TIMEOUT};
use model::{build_payload, decode_pipeline, parse_variables, write_output, PipelineRunResult};
use usage::USAGE;
use wait::wait_for_pipeline;

/// What `pipeline-run` says when the project path is missing; the usage
/// block has already gone to stderr by then.
const MISSING_PROJECT: &str =
    "pipeline-run: <project-path> required; pass e.g. faas/devine-app-integration-01-function";

/// The two independent deadlines of this command. The trigger bounds only
/// the POST; the poll loop gets its own budget and must never inherit it.
#[derive(Debug, Clone, Copy)]
struct Timeouts {
    /// Bounds only the POST that creates the pipeline.
    trigger: Duration,
    /// The `--wait` poll interval; the production value is 3s.
    poll_step: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Timeouts {
            trigger: Duration::from_secs(30),
            poll_step: Duration::from_secs(3),
        }
    }
}

/// `--base-url` wins, then the YAML config, then the documented default.
/// Go only reads the config when `--base-url` is empty.
fn resolve_base_url(flags: &RunFlags) -> Result<String, String> {
    if !flags.base_url.is_empty() {
        return Ok(flags.base_url.trim_end_matches('/').to_string());
    }
    let config = gitlab::load_watch_config(Some(&flags.config))
        .map_err(|error| format!("pipeline-run: load config: {error}"))?;
    let base_url = if config.base_url.is_empty() {
        gitlab::DEFAULT_BASE_URL.to_string()
    } else {
        config.base_url
    };
    Ok(base_url.trim_end_matches('/').to_string())
}

/// The command with its timeouts injected, so the tests can prove the
/// `--wait` loop does not inherit the trigger's deadline.
fn run(args: &[String], timeouts: Timeouts) -> Result<i32, String> {
    let (flags, positional) = parse_flags(args)?;
    let Some(project) = positional.first() else {
        eprint!("{USAGE}");
        return Err(MISSING_PROJECT.to_string());
    };
    let (token, token_source) = gitlab::resolve_pipeline_run_token(&flags.token)
        .map_err(|error| format!("pipeline-run: resolve token: {error}"))?;
    let base_url = resolve_base_url(&flags)?;
    let variables = parse_variables(&flags.variables);
    let payload = build_payload(&flags.reference, &variables);
    let url = format!(
        "{base_url}/api/v4/projects/{}/pipeline",
        gitlab::path_escape(project)
    );
    let headers = [
        ("PRIVATE-TOKEN", token.as_str()),
        ("Content-Type", "application/json"),
        ("Accept", "application/json"),
    ];
    let response = httpd::request(
        "POST",
        &url,
        &headers,
        Some(payload.as_slice()),
        timeouts.trigger,
    )
    .map_err(|error| format!("pipeline-run: request failed: {error}"))?;

    let mut result = PipelineRunResult::new(
        project,
        &flags.reference,
        variables,
        &base_url,
        &token_source,
    );
    if response.status >= 400 {
        // An HTTP error is a reported outcome, not a fatal one: Go writes
        // the result and returns a nil error, so the exit code stays 0.
        result.failed(&response);
        return write_output(&flags.output, flags.pretty, &result);
    }
    result.created(decode_pipeline(&response.body)?);

    if flags.wait {
        apply_wait(&flags, &mut result, &base_url, project, &token, timeouts)?;
    }
    write_output(&flags.output, flags.pretty, &result)
}

/// The `--wait` half: poll to a terminal state and fold the outcome into
/// `result`. A failed wait still reports what the trigger produced before
/// the error goes up, exactly like the Go original.
fn apply_wait(
    flags: &RunFlags,
    result: &mut PipelineRunResult,
    base_url: &str,
    project: &str,
    token: &str,
    timeouts: Timeouts,
) -> Result<(), String> {
    let pipeline_id = result.pipeline_id;
    let timeout = if flags.wait_timeout <= 0 {
        DEFAULT_WAIT_TIMEOUT
    } else {
        flags.wait_timeout
    };
    let outcome = wait_for_pipeline(
        base_url,
        project,
        pipeline_id,
        token,
        timeout,
        timeouts.poll_step,
    );
    let (final_status, elapsed) = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            let _ = write_output(&flags.output, flags.pretty, result);
            return Err(format!("pipeline-run: wait: {error}"));
        }
    };
    result.finished(&final_status, elapsed);
    if flags.expect_success && final_status != "success" {
        let _ = write_output(&flags.output, flags.pretty, result);
        return Err(format!(
            "pipeline-run: pipeline #{pipeline_id} on {} finished with \
             status {final_status:?} (--expect-success)",
            flags.reference
        ));
    }
    Ok(())
}

/// The `devine-utils-ci pipeline-run` entry point.
pub fn pipeline_run(args: &[String]) -> Result<i32, String> {
    run(args, Timeouts::default())
}

#[cfg(test)]
mod tests;
