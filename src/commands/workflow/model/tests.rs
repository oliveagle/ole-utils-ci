// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the topology model: the app lookup rules, the partial-merge
//! semantics a config file relies on, and the JSON shape `--output json`
//! publishes.

use super::*;
use crate::commands::workflow::defaults::default_workflow_config;

#[test]
fn empty_lookup_resolves_the_authoritative_dev_test_app() {
    let cfg = default_workflow_config();
    assert_eq!(
        cfg.find_test_app("").unwrap().name,
        "devine-test-004-function"
    );
}

#[test]
fn lookup_by_name_and_by_role() {
    let cfg = default_workflow_config();
    assert_eq!(
        cfg.find_test_app("devine-app-integration-02-function")
            .unwrap()
            .name,
        "devine-app-integration-02-function"
    );
    // The first app carrying the role wins, so integration-01 is the gate app.
    assert_eq!(
        cfg.find_test_app("integration").unwrap().name,
        "devine-app-integration-01-function"
    );
    assert_eq!(
        cfg.find_test_app("dev-test").unwrap().name,
        "devine-test-004-function"
    );
}

#[test]
fn unknown_lookup_lists_the_known_apps() {
    let cfg = default_workflow_config();
    let error = cfg.find_test_app("nope").unwrap_err();
    assert!(error.contains("no test app matching \"nope\""), "{error}");
    assert!(
        error.contains("devine-test-004-function(dev-test)"),
        "{error}"
    );
    assert!(
        error.contains("devine-app-integration-02-function(integration)"),
        "{error}"
    );
}

#[test]
fn empty_lookup_without_a_dev_test_app_fails_loudly() {
    let cfg = WorkflowConfig {
        test_apps: vec![TestApp {
            name: "solo".to_string(),
            role: ROLE_INTEGRATION.to_string(),
            ..TestApp::default()
        }],
        ..WorkflowConfig::default()
    };
    assert_eq!(
        cfg.find_test_app("").unwrap_err(),
        "workflow: no dev-test app in catalog"
    );
}

#[test]
fn enabled_groups_counts_only_the_live_clusters() {
    let cfg = default_workflow_config();
    let app = cfg
        .find_test_app("devine-app-integration-01-function")
        .unwrap();
    assert_eq!(app.prod_groups.len(), 3);
    assert_eq!(app.enabled_groups(), 2, "SGP stays disabled");
}

#[test]
fn apply_defaults_backfills_exactly_three_fields() {
    let mut cfg = WorkflowConfig {
        binary_repo: BinaryRepo {
            name: "x".to_string(),
            ..BinaryRepo::default()
        },
        ..WorkflowConfig::default()
    };
    cfg.apply_defaults();
    assert_eq!(cfg.binary_repo.remote, "origin");
    assert_eq!(cfg.binary_repo.version_file, "VERSION");
    assert_eq!(cfg.artifactory.min_size_b, 1_000_000);
    // Everything else is left blank on purpose.
    assert!(cfg.binary_repo.project_path.is_empty());
    assert!(cfg.artifactory.base_url.is_empty());
    assert!(cfg.test_apps.is_empty());
}

#[test]
fn merge_onto_lets_only_non_zero_fields_win() {
    let mut cfg = default_workflow_config();
    let entry: WorkflowFileEntry = serde_norway::from_str(
        r#"
binary_repo:
  name: renamed
  project: ""
  mirror_files: [pkg/version/VERSION]
  remote: upstream
  changelog_file: NOTES.md
artifactory:
  min_size_b: 2000000
  uat_path: ""
test_apps:
  - name: only-app
    project: faas/only-app
    role: dev-test
    version_url: http://only/version
"#,
    )
    .unwrap();
    entry.merge_onto(&mut cfg);

    assert_eq!(cfg.binary_repo.name, "renamed");
    // An explicit empty string never clears a default.
    assert_eq!(cfg.binary_repo.project_path, "devine/devine-golang-mcp-app");
    assert_eq!(cfg.binary_repo.remote, "upstream");
    assert_eq!(cfg.binary_repo.mirror_files, ["pkg/version/VERSION"]);
    // changelog_file is deliberately not part of the merge.
    assert_eq!(cfg.binary_repo.changelog_file, "CHANGELOG.md");
    assert_eq!(cfg.artifactory.min_size_b, 2_000_000);
    assert_eq!(
        cfg.artifactory.uat_path,
        "trip-generic-binary-uat-shanghai/devine-golang-mcp-app/uat/linux-amd64"
    );
    // A non-empty list replaces, it does not append.
    assert_eq!(cfg.test_apps.len(), 1);
    assert_eq!(cfg.test_apps[0].name, "only-app");
}

#[test]
fn into_config_names_a_new_workflow_and_applies_the_defaults() {
    let entry: WorkflowFileEntry = serde_norway::from_str(
        r#"
binary_repo:
  name: my-app
  project: ns/my-app
test_apps:
  - name: my-dev
    project: faas/my-dev
    role: dev-test
    version_url: http://x/version
"#,
    )
    .unwrap();
    let cfg = entry.into_config("my-app");
    assert_eq!(cfg.name, "my-app");
    assert_eq!(cfg.binary_repo.project_path, "ns/my-app");
    assert_eq!(cfg.binary_repo.version_file, "VERSION");
    assert_eq!(cfg.binary_repo.remote, "origin");
    assert_eq!(cfg.artifactory.min_size_b, 1_000_000);
    assert_eq!(cfg.test_apps.len(), 1);
}

#[test]
fn join_test_app_names_uses_the_name_role_spelling() {
    let cfg = default_workflow_config();
    assert_eq!(
        join_test_app_names(&cfg.test_apps[..2]),
        "devine-test-004-function(dev-test), devine-test-001-function(dev-test)"
    );
}

#[test]
fn test_app_json_keeps_declaration_order_and_omits_empty_fields() {
    let app = TestApp {
        name: "a".to_string(),
        project_path: "faas/a".to_string(),
        role: "dev-test".to_string(),
        debug: true,
        version_url: "http://a/version".to_string(),
        ..TestApp::default()
    };
    let json = serde_json::to_string(&app).unwrap();
    assert_eq!(
        json,
        r#"{"name":"a","project":"faas/a","role":"dev-test","debug":true,"version_url":"http://a/version"}"#
    );
}

#[test]
fn prod_group_json_always_carries_every_field() {
    let group = ProdGroup {
        id: 7,
        name: "SGP".to_string(),
        enabled: false,
    };
    assert_eq!(
        serde_json::to_string(&group).unwrap(),
        r#"{"id":7,"name":"SGP","enabled":false}"#
    );
}
