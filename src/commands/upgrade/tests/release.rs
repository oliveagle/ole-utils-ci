// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The release-fetching half of the `upgrade` tests: what a release offers,
//! and how a missing asset or a broken body is reported.

use super::*;

#[test]
fn available_assets_are_formatted_like_go_percent_v() {
    let links = vec![
        AssetLink {
            name: "a.gz".to_string(),
            url: "u".to_string(),
        },
        AssetLink {
            name: "b.gz".to_string(),
            url: "v".to_string(),
        },
    ];
    assert_eq!(list_asset_names(&links), "[a.gz b.gz]");
    assert_eq!(list_asset_names(&[]), "[]");
}

#[test]
fn fetch_release_info_reads_the_asset_list() {
    let body = br#"{"tag_name":"v1.2.3","assets":{"links":[{"name":"x.gz","url":"http://a/x"}]}}"#;
    let server = serve(|_, _| ("200 OK".to_string(), body.to_vec()));
    let cfg = config_for(&server.base_url, "v1.2.3");
    let links = fetch_release_info(&cfg).expect("decodes");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].name, "x.gz");
    assert_eq!(links[0].url, "http://a/x");
    server.shutdown();
}

#[test]
fn fetch_release_info_maps_404_and_a_broken_body() {
    let server = serve(|_, _| ("404 Not Found".to_string(), b"nope".to_vec()));
    let cfg = config_for(&server.base_url, "v9.9.9");
    let error = fetch_release_info(&cfg).expect_err("404");
    assert_eq!(error, "release v9.9.9 not found");
    server.shutdown();

    let server = serve(|_, _| ("200 OK".to_string(), b"<html>".to_vec()));
    let cfg = config_for(&server.base_url, "v9.9.9");
    let error = fetch_release_info(&cfg).expect_err("bad json");
    assert!(error.starts_with("decode response: "), "{error}");
    server.shutdown();
}

#[test]
fn fetch_latest_version_skips_release_candidates() {
    let body = br#"[{"tag_name":"v2.0.0-rc.1"},{"tag_name":"v1.9.0"}]"#;
    let server = serve(|_, _| ("200 OK".to_string(), body.to_vec()));
    let cfg = config_for(&server.base_url, "latest");
    assert_eq!(fetch_latest_version(&cfg).expect("latest"), "v1.9.0");
    server.shutdown();

    let server = serve(|_, _| {
        (
            "200 OK".to_string(),
            b"[{\"tag_name\":\"v1.0.0-rc.1\"}]".to_vec(),
        )
    });
    let cfg = config_for(&server.base_url, "latest");
    let error = fetch_latest_version(&cfg).expect_err("rc only");
    assert_eq!(error, "no stable release found");
    server.shutdown();
}

#[test]
fn a_release_without_our_artifact_lists_what_it_offers() {
    let server = serve(|_, origin| {
        let body = format!(
            r#"{{"assets":{{"links":[{{"name":"other.gz","url":"{origin}/artifact"}}]}}}}"#
        );
        ("200 OK".to_string(), body.into_bytes())
    });
    let root = tempfile::tempdir().expect("temp dir");
    let dir = root.path().join("out");
    let cfg = config_for(&server.base_url, "v1.2.3");
    let cfg = UpgradeConfig {
        dir: dir.to_string_lossy().to_string(),
        ..cfg
    };
    let error = run(cfg.clone()).expect_err("missing asset");
    assert!(
        error.contains("not found in release v1.2.3\nAvailable assets: [other.gz]"),
        "{error}"
    );
    assert!(!dir.exists(), "the install directory is created later");
    server.shutdown();
}
