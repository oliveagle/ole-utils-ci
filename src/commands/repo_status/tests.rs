// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::gitlab::{WatchConfig, WatchRepo};

fn config(repos: Vec<WatchRepo>) -> WatchConfig {
    WatchConfig {
        base_url: "https://gitlab.example".to_string(),
        repos,
        ..WatchConfig::default()
    }
}

#[test]
fn the_usage_block_starts_and_ends_where_go_does() {
    assert!(usage::USAGE.starts_with("devine-utils-ci repo-status — live pipeline status"));
    assert!(usage::USAGE.ends_with("1  fatal error (config / auth / network / bad args)\n"));
}

#[test]
fn a_project_entry_carries_its_kind_and_path() {
    let payload = list_repos_payload(&config(vec![WatchRepo {
        project: "devine/utils".to_string(),
        group: String::new(),
    }]));
    assert_eq!(payload["repos"][0]["kind"], "project");
    assert_eq!(payload["repos"][0]["path"], "devine/utils");
    assert!(payload["repos"][0].get("hint").is_none());
}

#[test]
fn a_group_entry_explains_how_to_expand_it() {
    let payload = list_repos_payload(&config(vec![WatchRepo {
        project: String::new(),
        group: "devine".to_string(),
    }]));
    assert_eq!(payload["repos"][0]["kind"], "group");
    assert!(payload["repos"][0]["hint"]
        .as_str()
        .unwrap_or_default()
        .contains("expand via"));
}

#[test]
fn a_repo_with_neither_project_nor_group_is_skipped() {
    let payload = list_repos_payload(&config(vec![WatchRepo::default()]));
    assert_eq!(payload["repos"], serde_json::json!([]));
}

#[test]
fn the_listing_keys_are_in_go_map_order() {
    let payload = list_repos_payload(&config(Vec::new()));
    let object = payload.as_object().expect("object");
    let keys: Vec<&String> = object.keys().collect();
    assert_eq!(keys, vec!["base_url", "hint", "repos"]);
}

#[test]
fn only_the_json_modes_need_go_escaping() {
    let mut flags = Flags::default();
    assert!(!uses_json(&flags));
    flags.output = "json".to_string();
    assert!(uses_json(&flags));
    flags.output = "text".to_string();
    assert!(uses_json(&flags));
    flags.output = "toon".to_string();
    assert!(!uses_json(&flags));
    flags.pretty = true;
    assert!(uses_json(&flags));
}

#[test]
fn a_payload_reaches_the_wire_with_go_escaping() {
    let value = serde_json::json!({"hint": "a < b & c > d"});
    let text = encode_value("json", false, &value).expect("encodes");
    let text = gojson::to_go_json(&text);
    assert!(text.contains("\\u003c"));
    assert!(text.contains("\\u0026"));
    assert!(text.contains("\\u003e"));
}
