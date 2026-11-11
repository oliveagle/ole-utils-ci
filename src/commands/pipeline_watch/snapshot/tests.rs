// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the JSON snapshot.

use super::*;
use crate::gitlab::WatchRefresh;

#[test]
fn the_snapshot_names_its_fields() {
    let config = WatchConfig {
        base_url: "https://git.test".to_string(),
        token: "t".to_string(),
        refresh: WatchRefresh {
            interval_seconds: 10,
        },
        per_repo: 7,
        local_root: "/tmp/clones".to_string(),
        repos: Vec::new(),
    };
    // No repos means no network: the snapshot still has its shape.
    let client = Client::new("https://git.test", "t");
    let text = snapshot_text(&config, &client, "config").expect("snapshot");
    assert!(text.contains("\"generated_at\""), "{text}");
    assert!(text.contains("\"token_source\": \"config\""), "{text}");
    assert!(
        text.contains("\"base_url\": \"https://git.test\""),
        "{text}"
    );
    assert!(text.contains("\"local_root\": \"/tmp/clones\""), "{text}");
    assert!(text.contains("\"per_repo\": 7"), "{text}");
    assert!(text.contains("\"rows\": []"), "{text}");
    assert!(
        !text.contains("\"errors\""),
        "an empty error list is dropped"
    );
    assert!(text.ends_with('\n'), "the encoder closes the document");
}
