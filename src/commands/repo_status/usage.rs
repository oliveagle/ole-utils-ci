// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The agent-facing usage block of `repo-status`.
//!
//! The Go original points the flag set's `Usage` at `printRepoStatusUsage`
//! rather than at `flag.PrintDefaults`, so this block is what a bad flag and
//! a missing project path both print. It is a verbatim copy: the worked
//! examples are the documentation agents copy-paste.

/// The usage block, ending with the newline `fmt.Fprintln` appends.
pub const USAGE: &str = r#"devine-utils-ci repo-status — live pipeline status (no cache; default output=TOON).

USAGE
    devine-utils-ci repo-status <project-path> [flags]
    devine-utils-ci repo-status --list-repos

FLAGS
    -config <path>       config file (default: ~/.devine/utils/ci/config.yaml)
    -base-url <url>      override GitLab base URL
    -pipelines <N>       recent pipelines to fetch (1..50, default 5)
    -pipeline <ID>       fetch ONE specific pipeline id (ignores -pipelines)
    --jobs               include per-job list on every pipeline
    --trace              include job trace bodies (implies --jobs; output can be MBs)
    --mrs                include open MRs list (with head pipeline status)
    --output FMT         output format: toon (default, compact/LLM-friendly)
                         | json (pretty, canonical) | text (single-line JSON, legacy)
    -o FMT               shorthand for --output
    --pretty             legacy alias for --output=json (pre-TOON callers)
    --list-repos         print configured repos and exit (no network)
    -timeout <sec>       per-request timeout in seconds (default 30)

OUTPUT
    A single JSON object. Top-level keys:
      project    { path, id, name, default_branch, web_url }
      head       { pipeline_id, status, ref, sha, updated_at, web_url }
      pipelines  [ { id, status, ref, sha, source, created_at, updated_at,
                     duration_seconds, web_url, jobs[]? } ]
      mrs        { open_count, head_pipeline_status, list[]? }
      commits    { last_1d }
      latest_tag { tag_name, commit_sha, released_at, source } (nil if never tagged)
      meta       { fetched_at, token_source, base_url, warnings[] }

    "status" uses the same EffectiveStatus the dashboard uses:
      success | warning | failed | running | pending | canceled | skipped | manual

AGENT USAGE GUIDE (read this first)

    1. If you don't know the project path, run FIRST:
           devine-utils-ci repo-status --list-repos --pretty
       to enumerate all configured repos. Use their "path" verbatim as
       <project-path>.

    2. To answer "is my last push green?" — the cheapest query:
           devine-utils-ci repo-status <path> --pipelines 1
       Read .head.status and .head.pipeline_id.

    3. To investigate a specific failed pipeline:
           devine-utils-ci repo-status <path> --pipeline 60977145 --jobs
       Read .pipelines[0].jobs[]; find status="failed" entries; read
       .name / .stage / .id / .failure_reason.

    4. To pull the actual failure log:
           devine-utils-ci repo-status <path> --pipeline 60977145 --trace
       Grep .pipelines[0].jobs[*].trace for the error. Warning: trace
       bodies can be MBs; prefer --jobs first to identify the failing
       job, then a targeted query.

    5. To check open MRs (are there PRs waiting?):
           devine-utils-ci repo-status <path> --mrs
       Read .mrs.open_count and .mrs.list[].{iid, title, web_url,
       source_branch, head_pipeline_status}.
       .mrs.list is sorted "look here first": failed head pipeline →
       canceled → running/pending/warning → success → no pipeline,
       newest IID first within the same bucket (same priority as the
       dashboard's MR-dot rollup).

    6. Every call is LIVE — no cache. Time-sensitive; treat the response
       as ground truth for the .meta.fetched_at moment only.

EXAMPLES

    # Configured repo list
    devine-utils-ci repo-status --list-repos --pretty

    # Last 5 pipelines of one repo, no jobs
    devine-utils-ci repo-status devine/devine_utils --pretty

    # Debug the head pipeline
    devine-utils-ci repo-status devine/devine_utils --pipelines 1 --jobs

    # Full detail on a specific pipeline (needs the pipeline ID)
    devine-utils-ci repo-status devine/devine_utils --pipeline 60977145 --jobs --pretty

    # Everything (open MRs + head pipeline + last 3 pipelines with jobs)
    devine-utils-ci repo-status devine/devine_utils --pipelines 3 --jobs --mrs --pretty

EXIT CODES
    0  success
    1  fatal error (config / auth / network / bad args)
"#;
