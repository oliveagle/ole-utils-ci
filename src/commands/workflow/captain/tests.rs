// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the Captain client: release parsing, the image lookup and
//! the wait that must follow the release id this run created.

mod server;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use server::{Server, RELEASES};

use super::*;

#[test]
fn releases_parses_the_envelope() {
    let server = Server::start(|seen| {
        assert_eq!(seen.path, "/releases?group_id=7&per_page=10");
        (200, RELEASES.to_string())
    });
    let client = CaptainClient::new(&server.base, "tok");
    let releases = client.releases(7).unwrap();
    assert_eq!(releases.len(), 2);
    assert_eq!(releases[0].id, 2);
    assert_eq!(releases[0].status, "SUCCESS");
    assert_eq!(releases[0].diff[0].key, "Image URL");
}

#[test]
fn the_latest_fat_image_is_the_newest_success_with_an_image() {
    let server = Server::start(|_| (200, RELEASES.to_string()));
    let client = CaptainClient::new(&server.base, "");
    assert_eq!(client.latest_fat_image(7).unwrap(), "gitlab-ci/1:main-new");
}

#[test]
fn a_group_without_an_image_release_explains_itself() {
    let server = Server::start(|_| {
        (
            200,
            r#"{"result":[{"id":1,"status":"PENDING","diff":[]}]}"#.to_string(),
        )
    });
    let client = CaptainClient::new(&server.base, "");
    let error = client.latest_fat_image(9).unwrap_err();
    assert!(
        error.contains("no SUCCESS release with Image URL in FAT group 9"),
        "{error}"
    );
    assert!(error.contains("latest 1 releases"), "{error}");
}

#[test]
fn auto_deploy_sends_the_alphabetical_body_and_reads_a_nested_id() {
    // Regression: Captain answers {"release":{"id":N}}, which the first
    // parser missed, leaving the release PENDING forever.
    let server = Server::start(|seen| {
        assert_eq!(seen.method, "POST");
        assert_eq!(seen.path, "/releases/auto_deploy");
        (
            200,
            r#"{"release":{"id":486765,"status":"PENDING"}}"#.to_string(),
        )
    });
    let client = CaptainClient::new(&server.base, "tok");
    assert_eq!(
        client
            .auto_deploy(1044275, "gitlab-ci/310013516:main-abc")
            .unwrap(),
        486765
    );
    let request = &server.requests()[0];
    let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["id"], 1044275);
    assert_eq!(body["image_url"], "gitlab-ci/310013516:main-abc");
    assert_eq!(body["resource"], "RESOURCE_GROUP");
    assert_eq!(body["batches"][0]["percent"], 100);
    let keys: Vec<&str> = request
        .body
        .split(',')
        .filter_map(|part| part.split(':').next())
        .collect();
    assert!(
        keys[0].contains("batches"),
        "Go marshals maps alphabetically: {keys:?}"
    );
}

#[test]
fn auto_deploy_accepts_every_documented_id_shape() {
    for body in [
        r#"{"id":5}"#,
        r#"{"release_id":6}"#,
        r#"{"data":{"id":7}}"#,
        r#"{"data":{"release":{"release_id":8}}}"#,
    ] {
        let body: &'static str = Box::leak(body.to_string().into_boxed_str());
        let server = Server::start(move |_| (200, body.to_string()));
        let client = CaptainClient::new(&server.base, "");
        let id = client.auto_deploy(1, "img").unwrap();
        assert!(id >= 5, "{body} -> {id}");
    }
}

#[test]
fn an_id_less_auto_deploy_response_is_an_error_with_the_body() {
    let server = Server::start(|_| (200, r#"{"status":"PENDING"}"#.to_string()));
    let client = CaptainClient::new(&server.base, "");
    let error = client.auto_deploy(1, "img").unwrap_err();
    assert!(
        error.contains("auto_deploy response had no release id"),
        "{error}"
    );
    assert!(error.contains("PENDING"), "{error}");
}

#[test]
fn both_credential_headers_are_sent_when_a_token_is_present() {
    let server = Server::start(|_| (200, r#"{"result":[]}"#.to_string()));
    let client = CaptainClient::new(&server.base, "sekret");
    client.releases(1).unwrap();
    let request = &server.requests()[0];
    assert_eq!(request.header("Authorization"), "Bearer sekret");
    assert_eq!(request.header("X-User-Token"), "sekret");
}

#[test]
fn no_credential_headers_are_sent_without_a_token() {
    let server = Server::start(|_| (200, r#"{"result":[]}"#.to_string()));
    let client = CaptainClient::new(&server.base, "");
    client.releases(1).unwrap();
    let request = &server.requests()[0];
    assert!(request.header("Authorization").is_empty());
    assert!(request.header("X-User-Token").is_empty());
}

#[test]
fn an_http_error_carries_the_status_and_body() {
    let server = Server::start(|_| (401, "no auth".to_string()));
    let client = CaptainClient::new(&server.base, "");
    let error = client.releases(1).unwrap_err();
    assert!(error.contains("HTTP 401"), "{error}");
    assert!(error.contains("no auth"), "{error}");
}

#[test]
fn rollout_posts_to_the_release_path() {
    let server = Server::start(|_| (200, "{}".to_string()));
    let client = CaptainClient::new(&server.base, "tok");
    client.rollout(42).unwrap();
    let request = &server.requests()[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/releases/42/rollout");
}

#[test]
fn status_of_an_unlisted_release_is_empty() {
    let server = Server::start(|_| {
        (
            200,
            r#"{"result":[{"id":1,"status":"SUCCESS"}]}"#.to_string(),
        )
    });
    let client = CaptainClient::new(&server.base, "");
    assert_eq!(client.release_status(7, 1).unwrap(), "SUCCESS");
    assert_eq!(client.release_status(7, 99).unwrap(), "");
}

#[test]
fn the_wait_rolls_out_the_pending_release_it_created() {
    let rolled = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&rolled);
    let server = Server::start(move |seen| {
        if seen.method == "POST" && seen.path == "/releases/9/rollout" {
            flag.store(true, Ordering::SeqCst);
            return (200, "{}".to_string());
        }
        // The older SUCCESS release carries the same image; the new one starts
        // PENDING and turns SUCCESS once it has been rolled out.
        let status = if flag.load(Ordering::SeqCst) {
            "SUCCESS"
        } else {
            "PENDING"
        };
        (
            200,
            format!(
                r#"{{"result":[{{"id":8,"status":"SUCCESS"}},{{"id":9,"status":"{status}"}}]}}"#
            ),
        )
    });
    let client = CaptainClient::new(&server.base, "");
    let status = client
        .wait_release_by_id(1044275, 9, 30, false, Duration::from_millis(1))
        .unwrap();
    assert_eq!(status, "SUCCESS");
    assert!(rolled.load(Ordering::SeqCst));
    let rolled_path = server
        .requests()
        .iter()
        .any(|request| request.path == "/releases/9/rollout");
    assert!(
        rolled_path,
        "the release this run created must be rolled out"
    );
}

#[test]
fn a_release_that_fails_is_reported_with_its_id() {
    let server = Server::start(|_| {
        (
            200,
            r#"{"result":[{"id":9,"status":"FAILED","diff":[]}]}"#.to_string(),
        )
    });
    let client = CaptainClient::new(&server.base, "");
    let error = client
        .wait_release_by_id(1, 9, 5, false, Duration::from_millis(1))
        .unwrap_err();
    assert_eq!(error, "release 9 ended in FAILED");
}

#[test]
fn a_stuck_release_times_out_with_its_last_status() {
    let server = Server::start(|_| {
        (
            200,
            r#"{"result":[{"id":9,"status":"RUNNING"}]}"#.to_string(),
        )
    });
    let client = CaptainClient::new(&server.base, "");
    let error = client
        .wait_release_by_id(1, 9, 0, false, Duration::from_millis(1))
        .unwrap_err();
    assert!(error.contains("not SUCCESS after 0s"), "{error}");
    assert!(error.contains("last status \"RUNNING\""), "{error}");
}
