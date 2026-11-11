// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for `pipeline-watch`.
//!
//! Every networked test stands up a loopback GitLab, so the suite runs
//! without touching the real instance.

mod mock;

use mock::{serve, Reply};

use super::cache::Cache;
use super::client::Client;
use super::fetch;
use super::rows::RepoRow;

/// The project the fake GitLab knows about.
const PROJECT: &str = "devine%2Futils";

/// Answers the endpoints the dashboard reads.
fn handler(request: &mock::Seen) -> Reply {
    let path = request
        .path
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    if path == format!("/api/v4/projects/{PROJECT}") {
        return Reply::json(
            "{\"id\":7,\"path_with_namespace\":\"devine/utils\",\
             \"web_url\":\"https://git.test/devine/utils\",\
             \"default_branch\":\"main\",\"name\":\"utils\",\"archived\":false}",
        );
    }
    if path == format!("/api/v4/projects/{PROJECT}/pipelines") {
        return Reply::json(PIPELINES);
    }
    if path.contains("/pipelines/") && path.ends_with("/jobs") {
        return Reply::json(JOBS);
    }
    if path == format!("/api/v4/projects/{PROJECT}/merge_requests") {
        return Reply::json(MERGE_REQUESTS).with_headers(&[("X-Total", "2")]);
    }
    if path == format!("/api/v4/projects/{PROJECT}/repository/commits") {
        return Reply::json("[]").with_headers(&[("X-Total", "7")]);
    }
    if path == format!("/api/v4/projects/{PROJECT}/releases") {
        return Reply::json(RELEASES);
    }
    if path == format!("/api/v4/projects/{PROJECT}/repository/branches/main") {
        return Reply::json(
            "{\"name\":\"main\",\"default\":true,\"commit\":{\"id\":\"aaaa1111bbbb2222\"}}",
        );
    }
    Reply::missing()
}

/// Two pipelines: one green, one failed.
const PIPELINES: &str = "[{\"id\":42,\"project_id\":7,\"ref\":\"main\",\
     \"sha\":\"aaaa1111\",\"status\":\"success\",\"source\":\"push\",\
     \"created_at\":\"2026-11-11T08:00:00.000Z\",\
     \"updated_at\":\"2026-11-11T08:10:00.000Z\",\"duration\":0,\"web_url\":\
     \"https://git.test/devine/utils/-/pipelines/42\"},\
     {\"id\":41,\"project_id\":7,\"ref\":\"main\",\"sha\":\"cccc3333\",\
     \"status\":\"failed\",\"source\":\"merge_request_event\",\
     \"created_at\":\"2026-11-10T08:00:00.000Z\",\
     \"updated_at\":\"2026-11-10T08:10:00.000Z\",\"duration\":0,\"web_url\":\
     \"https://git.test/devine/utils/-/pipelines/41\"}]";

/// Three jobs, one of them failed.
const JOBS: &str = "[{\"id\":1,\"status\":\"success\",\"name\":\"build\"},\
     {\"id\":2,\"status\":\"success\",\"name\":\"test\"},\
     {\"id\":3,\"status\":\"failed\",\"name\":\"lint\"}]";

/// One open merge request with a running head pipeline.
const MERGE_REQUESTS: &str = "[{\"id\":9,\"head_pipeline\":{\"id\":43,\"status\":\"running\",\
     \"detailed_status\":{\"group\":\"running\"}}}]";

/// One release.
const RELEASES: &str = "[{\"tag_name\":\"v0.3.3\",\"released_at\":\"2026-11-01T00:00:00.000Z\",\
     \"commit\":{\"id\":\"aaaa1111bbbb2222\"}}]";

/// A configuration that watches one project.
fn config(base_url: &str) -> crate::gitlab::WatchConfig {
    crate::gitlab::WatchConfig {
        base_url: base_url.to_string(),
        token: "t".to_string(),
        refresh: crate::gitlab::WatchRefresh {
            interval_seconds: 10,
        },
        per_repo: 7,
        local_root: String::new(),
        repos: vec![crate::gitlab::WatchRepo {
            group: String::new(),
            project: "devine/utils".to_string(),
        }],
    }
}

/// The first row of a fetch result.
fn first_row(result: &fetch::FetchResult) -> &RepoRow {
    result.rows.first().expect("one row")
}

#[test]
fn the_dashboard_fills_one_row_per_project() {
    let server = serve(handler);
    let client = Client::new(&server.base_url, "t");
    let mut cache = Cache::new();
    let result = fetch::fetch_dashboard(
        &client,
        &config(&server.base_url),
        &mut cache,
        chrono::Utc::now(),
    );
    server.shutdown();

    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert_eq!(result.rows.len(), 1);
    let row = first_row(&result);
    assert_eq!(row.project, "devine/utils");
    assert_eq!(row.mr_count, 2);
    assert_eq!(row.commit_count_1d, 7);
    assert_eq!(row.success_count, 1);
    assert_eq!(row.failure_count, 1);
    assert_eq!(row.pipeline_dots.len(), 2);
    assert_eq!(row.pipeline_dots[0].job_count, 3);
    assert_eq!(row.last_total_jobs, 3);
    assert_eq!(row.last_failed_jobs, 1);
    assert_eq!(row.last_status, "success");
    assert_eq!(row.latest_tag, "v0.3.3");
    assert_eq!(row.main_commit_sha, "aaaa1111bbbb2222");
    assert_eq!(row.pipeline_id, 42);
}

#[test]
fn the_merge_request_pipeline_status_reaches_the_row() {
    let server = serve(handler);
    let client = Client::new(&server.base_url, "t");
    let mut cache = Cache::new();
    let result = fetch::fetch_dashboard(
        &client,
        &config(&server.base_url),
        &mut cache,
        chrono::Utc::now(),
    );
    server.shutdown();
    assert_eq!(first_row(&result).mr_pipeline_status, "running");
}

#[test]
fn the_dashboard_asks_the_documented_urls() {
    let server = serve(handler);
    let client = Client::new(&server.base_url, "t");
    let mut cache = Cache::new();
    let _ = fetch::fetch_dashboard(
        &client,
        &config(&server.base_url),
        &mut cache,
        chrono::Utc::now(),
    );
    let paths = server.paths();
    let methods: Vec<String> = server.requests().iter().map(|r| r.method.clone()).collect();
    server.shutdown();
    assert!(methods.iter().all(|method| method == "GET"));

    assert!(paths
        .iter()
        .any(|path| path == "/api/v4/projects/devine%2Futils"));
    assert!(paths
        .iter()
        .any(|path| path == "/api/v4/projects/devine%2Futils/pipelines?per_page=7&page=1"));
    assert!(paths
        .iter()
        .any(|path| path == "/api/v4/projects/devine%2Futils/pipelines/42/jobs?per_page=100"));
    assert!(paths.iter().any(|path| path
        == "/api/v4/projects/devine%2Futils/merge_requests?state=opened&per_page=20&page=1"));
    assert!(paths
        .iter()
        .any(|path| path == "/api/v4/projects/devine%2Futils/repository/branches/main"));
}

#[test]
fn a_failing_project_does_not_blank_the_dashboard() {
    let server = serve(|request: &mock::Seen| {
        let path = request.path.split('?').next().unwrap_or_default();
        if path == "/api/v4/projects/devine%2Futils" {
            return Reply::json(
                "{\"id\":7,\"path_with_namespace\":\"devine/utils\",\
                 \"default_branch\":\"main\",\"name\":\"utils\"}",
            );
        }
        Reply::missing()
    });
    let client = Client::new(&server.base_url, "t");
    let mut cache = Cache::new();
    let result = fetch::fetch_dashboard(
        &client,
        &config(&server.base_url),
        &mut cache,
        chrono::Utc::now(),
    );
    server.shutdown();

    assert_eq!(result.rows.len(), 1);
    assert!(!result.errors.is_empty());
    assert!(result
        .errors
        .iter()
        .any(|error| error.contains("pipelines")));
}

#[test]
fn the_second_round_is_served_from_the_cache() {
    let server = serve(handler);
    let client = Client::new(&server.base_url, "t");
    let mut cache = Cache::new();
    let _ = fetch::fetch_dashboard(
        &client,
        &config(&server.base_url),
        &mut cache,
        chrono::Utc::now(),
    );
    let first = server.paths().len();
    let _ = fetch::fetch_dashboard(
        &client,
        &config(&server.base_url),
        &mut cache,
        chrono::Utc::now(),
    );
    let second = server.paths().len();
    server.shutdown();
    assert_eq!(first, second, "the second round hit the network again");
}
