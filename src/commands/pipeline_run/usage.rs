// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The hand-written usage block `pipeline-run` prints when the project
//! path is missing.
//!
//! Go keeps this text as a raw literal inside `printPipelineRunUsage` and
//! renders it with `fmt.Fprintln`, so the block ends with exactly one
//! newline. It lives in its own file only to keep `pipeline_run.rs` inside
//! the file-length budget.

/// Byte-identical to the Go original's `printPipelineRunUsage`, newline
/// included — the 1859 bytes `Fprintln` writes to stderr.
pub(super) const USAGE: &str = r#"devine-utils-ci pipeline-run — trigger a GitLab pipeline.

USAGE
    devine-utils-ci pipeline-run <project-path> [flags]

FLAGS
    --ref <branch>       branch or tag to run on (default: main)
    --var KEY=VALUE      pipeline variable (repeatable)
    --token <token>      GitLab personal access token (default: $GITLAB_TOKEN)
    --base-url <url>     override GitLab base URL
    --config <path>      config file (default: ~/.devine/utils/ci/config.yaml)
    --output <fmt>       output format: toon (default) | json | text
    --wait               poll until pipeline finishes (default: fire-and-forget)
    --wait-timeout <N>   max seconds to wait with --wait (default: 600)
    --expect-success     with --wait: exit non-zero unless the pipeline ends in success

TOKEN RESOLUTION (in priority order):
    1. --token flag
    2. $GITLAB_TOKEN environment variable
    3. $DEVINE_GIT_GROUP_ACCESS_TOKEN environment variable
    4. dotvault get DEVINE_GIT_GROUP_ACCESS_KEY

EXAMPLES
    # Trigger pipeline on main branch (default)
    devine-utils-ci pipeline-run faas/devine-app-integration-01-function

    # Trigger on a specific branch
    devine-utils-ci pipeline-run faas/devine-app-integration-01-function --ref dev

    # Trigger with pipeline variables
    devine-utils-ci pipeline-run faas/devine-app-integration-01-function --var DEPLOY_ENV=staging --var DEBUG=true

    # Trigger and wait for completion
    devine-utils-ci pipeline-run faas/devine-app-integration-01-function --wait

    # Use explicit token
    devine-utils-ci pipeline-run faas/devine-app-integration-01-function --token glpat-xxxx

    # JSON output for scripting
    devine-utils-ci pipeline-run faas/devine-app-integration-01-function --output json

EXIT CODES
    0  pipeline triggered successfully (or completed if --wait)
    1  fatal error (auth, network, bad args)
"#;
