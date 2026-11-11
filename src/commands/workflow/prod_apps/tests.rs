// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the `prod-verify` selection rules.

use super::*;
use crate::commands::workflow::defaults::default_workflow_config;

#[test]
fn stable_versions_are_plain_triples() {
    for good in ["0.1.24", "1.2.3", "10.20.30", " 1.2.3 "] {
        assert!(is_stable_version(good), "{good:?} should be stable");
    }
    for bad in [
        "0.1.25-rc.6",
        "1.2.3-rc.1",
        "1.2",
        "1.2.3.4",
        "",
        "v1.2.3",
        "1.2.x",
        "1..3",
    ] {
        assert!(!is_stable_version(bad), "{bad:?} should be refused");
    }
}

#[test]
fn all_catalog_apps_are_selected_in_preferred_order() {
    let cfg = default_workflow_config();
    let apps = select_apps(&cfg, "");
    let names: Vec<&str> = apps.iter().map(|app| app.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "devine-app-integration-02-function",
            "devine-app-integration-01-function",
            "devine-test-004-function",
            "devine-test-001-function",
        ]
    );
}

#[test]
fn a_filter_narrows_to_the_named_apps() {
    let cfg = default_workflow_config();
    let one = select_apps(&cfg, "devine-app-integration-02-function");
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].name, "devine-app-integration-02-function");

    let two = select_apps(
        &cfg,
        " devine-test-004-function , devine-test-001-function ",
    );
    assert_eq!(two.len(), 2);
    // Preferred order still decides, not the order the filter listed them.
    assert_eq!(two[0].name, "devine-test-004-function");
    assert_eq!(two[1].name, "devine-test-001-function");

    assert!(select_apps(&cfg, "does-not-exist").is_empty());
}

#[test]
fn apps_outside_the_preferred_order_are_appended() {
    let mut cfg = default_workflow_config();
    cfg.test_apps.push(TestApp {
        name: "extra-app".to_string(),
        role: "playground".to_string(),
        ..TestApp::default()
    });
    let apps = select_apps(&cfg, "");
    assert_eq!(apps.len(), 5);
    assert_eq!(apps[4].name, "extra-app");
}

#[test]
fn an_empty_filter_entry_is_ignored() {
    let cfg = default_workflow_config();
    // Trailing commas must not turn into a match-everything rule.
    let apps = select_apps(&cfg, " , ");
    assert!(apps.is_empty());
}
