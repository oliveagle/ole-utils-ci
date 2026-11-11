// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for `workflow list`, `workflow apps` and the two rc triggers.

use super::*;
use crate::commands::workflow::defaults::default_workflow_config;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

/// A context that is not a checkout, so no VERSION is picked up.
fn ctx() -> Context {
    Context {
        cwd: std::path::PathBuf::from("/nonexistent"),
        home: String::new(),
    }
}

/// A registry with the built-in plus one user workflow.
fn registry() -> Registry {
    let mut reg = Registry::builtin();
    reg.insert(WorkflowConfig {
        name: "my-app".to_string(),
        binary_repo: crate::commands::workflow::model::BinaryRepo {
            name: "my-app".to_string(),
            project_path: "ns/my-app".to_string(),
            version_file: "VERSION".to_string(),
            ..Default::default()
        },
        test_apps: vec![
            crate::commands::workflow::model::TestApp {
                name: "my-dev".to_string(),
                project_path: "faas/my-dev".to_string(),
                role: "dev-test".to_string(),
                version_url: "http://x/version".to_string(),
                ..Default::default()
            },
            crate::commands::workflow::model::TestApp {
                name: "my-int".to_string(),
                project_path: "faas/my-int".to_string(),
                role: "integration".to_string(),
                version_url: "http://y/version".to_string(),
                ..Default::default()
            },
        ],
        ..Default::default()
    });
    reg
}

#[test]
fn list_rows_cover_every_workflow_in_name_order() {
    let rows = list_rows(&registry());
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, DEFAULT_WORKFLOW_NAME);
    assert!(rows[0].builtin);
    assert_eq!(rows[0].binary_repo, "devine-golang-mcp-app");
    assert_eq!(rows[0].project, "devine/devine-golang-mcp-app");
    assert_eq!(rows[0].test_apps.len(), 4);
    assert_eq!(rows[0].roles[0], "dev-test");

    assert_eq!(rows[1].name, "my-app");
    assert!(!rows[1].builtin);
    assert_eq!(rows[1].test_apps, ["my-dev", "my-int"]);
    assert_eq!(rows[1].roles, ["dev-test", "integration"]);
}

#[test]
fn list_rows_serialise_with_the_documented_field_order() {
    let rows = list_rows(&registry());
    let text = serde_json::to_string(&rows[1]).unwrap();
    assert_eq!(
        text,
        r#"{"name":"my-app","binary_repo":"my-app","project":"ns/my-app","builtin":false,"test_apps":["my-dev","my-int"],"roles":["dev-test","integration"]}"#
    );
}

#[test]
fn list_text_and_json_both_succeed() {
    let reg = registry();
    assert_eq!(workflow_list(&[], &reg).unwrap(), 0);
    assert_eq!(
        workflow_list(&args(&["--output", "json"]), &reg).unwrap(),
        0
    );
}

#[test]
fn list_rejects_positional_arguments() {
    let error = workflow_list(&args(&["extra"]), &registry()).unwrap_err();
    assert_eq!(error, "list: unexpected positional args [extra]");
}

#[test]
fn apps_renders_the_catalog_and_rejects_positionals() {
    let cfg = default_workflow_config();
    assert_eq!(workflow_apps(&[], &cfg).unwrap(), 0);
    assert_eq!(
        workflow_apps(&args(&["--output", "json"]), &cfg).unwrap(),
        0
    );
    let error = workflow_apps(&args(&["stray"]), &cfg).unwrap_err();
    assert_eq!(error, "apps: unexpected positional args [stray]");
}

#[test]
fn test_rc_dry_run_resolves_the_dev_test_app() {
    let cfg = default_workflow_config();
    assert_eq!(
        workflow_test_rc(&args(&["--dry-run"]), &cfg, &ctx()).unwrap(),
        0
    );
}

#[test]
fn test_rc_rejects_a_positional_argument() {
    let cfg = default_workflow_config();
    let error = workflow_test_rc(&args(&["--dry-run", "oops"]), &cfg, &ctx()).unwrap_err();
    assert_eq!(error, "test-rc: unexpected positional args [oops]");
}

#[test]
fn integration_rc_dry_run_honours_the_app_flag() {
    let cfg = default_workflow_config();
    assert_eq!(
        workflow_integration_rc(
            &args(&["--app", "devine-app-integration-02-function", "--dry-run"]),
            &cfg,
            &ctx()
        )
        .unwrap(),
        0
    );
}

#[test]
fn integration_rc_rejects_an_unknown_app() {
    let cfg = default_workflow_config();
    let error =
        workflow_integration_rc(&args(&["--app", "nope", "--dry-run"]), &cfg, &ctx()).unwrap_err();
    assert!(error.contains("no test app matching \"nope\""), "{error}");
}

#[test]
fn the_parser_accepts_every_documented_flag() {
    let (flags, rest) = parse_test_rc(&args(&[
        "--wait",
        "--wait-timeout",
        "abc",
        "--poll-timeout",
        "30",
        "--poll-version",
        "--verbose",
    ]));
    assert!(flags.wait);
    assert!(flags.poll_version);
    assert_eq!(flags.wait_timeout, 0, "a non-numeric timeout reads as 0");
    assert_eq!(flags.poll_timeout, 30);
    assert!(flags.common.verbose);
    assert!(rest.is_empty());
}

#[test]
fn the_integration_parser_keeps_the_app() {
    let (flags, rest) = parse_integration_rc(&args(&["--app", "integration", "--wait"]));
    assert_eq!(flags.app, "integration");
    assert!(flags.wait);
    assert!(rest.is_empty());
}

#[test]
fn the_list_hint_points_at_the_top_level_command() {
    assert!(LIST_HINT.contains("devine-utils-ci workflow list"));
}
