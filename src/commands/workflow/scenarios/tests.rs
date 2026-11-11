// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for `workflow scenarios`, exercised against loopback apps that
//! speak the same MCP shape the FAAS apps do.

mod mcp;

use mcp::McpApp;

use super::*;
use crate::commands::workflow::model::{TestApp, WorkflowConfig};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

/// Every scenario tool passing.
fn all_pass() -> Vec<(&'static str, &'static str)> {
    SCENARIO_TOOLS.iter().map(|tool| (*tool, "PASS")).collect()
}

/// One integration app whose FAT and PROD bases point at the servers.
fn config(fat: &McpApp, prod: &McpApp) -> WorkflowConfig {
    WorkflowConfig {
        name: "test".to_string(),
        test_apps: vec![TestApp {
            name: "devine-app-integration-01-function".to_string(),
            project_path: "faas/devine-app-integration-01-function".to_string(),
            role: ROLE_INTEGRATION.to_string(),
            version_url: format!("{}/version", fat.base),
            prod_url: prod.base.clone(),
            ..TestApp::default()
        }],
        ..WorkflowConfig::default()
    }
}

#[test]
fn base_urls_strip_the_version_suffix_and_trailing_slashes() {
    let app = TestApp {
        version_url: "http://a.fws.example/version".to_string(),
        prod_url: "http://a.faas.example/".to_string(),
        ..TestApp::default()
    };
    assert_eq!(
        scenario_base_url(&app, "fat").unwrap(),
        "http://a.fws.example"
    );
    assert_eq!(
        scenario_base_url(&app, "prod").unwrap(),
        "http://a.faas.example"
    );
    assert_eq!(scenario_base_url(&TestApp::default(), "prod"), None);
    assert_eq!(scenario_base_url(&app, "staging"), None);
}

#[test]
fn a_sweep_where_everything_passes_succeeds() {
    let fat = McpApp::start(all_pass());
    let prod = McpApp::start(all_pass());
    let cfg = config(&fat, &prod);
    assert_eq!(
        workflow_scenarios(&args(&["--env", "all"]), &cfg).unwrap(),
        0
    );
    assert_eq!(fat.hits(), SCENARIO_TOOLS.len());
    assert_eq!(prod.hits(), SCENARIO_TOOLS.len());
}

#[test]
fn a_single_fail_verdict_fails_the_run() {
    let mut tools = all_pass();
    tools[0] = ("it_downstream_routing", "FAIL");
    let fat = McpApp::start(tools);
    let prod = McpApp::start(all_pass());
    let cfg = config(&fat, &prod);
    let error = workflow_scenarios(&args(&["--env", "fat"]), &cfg).unwrap_err();
    assert!(error.contains("did not pass"), "{error}");
    assert!(!error.contains("--strict"), "{error}");
}

#[test]
fn missing_tools_only_fail_under_strict() {
    let fat = McpApp::start(Vec::new());
    let prod = McpApp::start(Vec::new());
    let cfg = config(&fat, &prod);
    workflow_scenarios(&args(&["--env", "fat"]), &cfg).expect("MISSING is a state, not a failure");
    let error = workflow_scenarios(&args(&["--env", "fat", "--strict"]), &cfg).unwrap_err();
    assert!(
        error.contains("scenario tool(s) missing (--strict)"),
        "{error}"
    );
}

#[test]
fn env_fat_never_contacts_prod() {
    let fat = McpApp::start(all_pass());
    let prod = McpApp::start(all_pass());
    let cfg = config(&fat, &prod);
    workflow_scenarios(&args(&["--env", "fat"]), &cfg).unwrap();
    assert_eq!(prod.hits(), 0, "--env fat must not touch PROD");
    assert_eq!(fat.hits(), SCENARIO_TOOLS.len());
}

#[test]
fn an_unknown_env_is_rejected_before_any_request() {
    let fat = McpApp::start(all_pass());
    let prod = McpApp::start(all_pass());
    let cfg = config(&fat, &prod);
    let error = workflow_scenarios(&args(&["--env", "staging"]), &cfg).unwrap_err();
    assert!(error.contains("--env must be fat|prod|all"), "{error}");
    assert_eq!(fat.hits(), 0);
}

#[test]
fn a_dry_run_reports_the_matrix_without_calling_anything() {
    let fat = McpApp::start(all_pass());
    let prod = McpApp::start(all_pass());
    let cfg = config(&fat, &prod);
    assert_eq!(workflow_scenarios(&args(&["--dry-run"]), &cfg).unwrap(), 0);
    assert_eq!(fat.hits(), 0);
    assert_eq!(prod.hits(), 0);
}

#[test]
fn a_single_tool_and_app_can_be_selected() {
    let fat = McpApp::start(all_pass());
    let prod = McpApp::start(all_pass());
    let cfg = config(&fat, &prod);
    let flags = args(&[
        "--env",
        "prod",
        "--app",
        "devine-app-integration-01-function",
        "--tool",
        "it_mcp_chain",
    ]);
    assert_eq!(workflow_scenarios(&flags, &cfg).unwrap(), 0);
    assert_eq!(prod.hits(), 1);
    assert_eq!(fat.hits(), 0);
}

#[test]
fn an_app_filter_matching_nothing_is_an_error() {
    let cfg = config(&McpApp::start(Vec::new()), &McpApp::start(Vec::new()));
    let error = workflow_scenarios(&args(&["--app", "nope"]), &cfg).unwrap_err();
    assert!(
        error.contains("no integration app in catalog matches"),
        "{error}"
    );
}

#[test]
fn only_integration_apps_are_swept() {
    let mut cfg = config(&McpApp::start(Vec::new()), &McpApp::start(Vec::new()));
    cfg.test_apps.push(TestApp {
        name: "a-dev-test".to_string(),
        role: "dev-test".to_string(),
        version_url: "http://dev/version".to_string(),
        ..TestApp::default()
    });
    let selected = select_apps(&cfg, "");
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].role, ROLE_INTEGRATION);
}

#[test]
fn a_catalog_without_any_base_url_is_an_error() {
    let cfg = WorkflowConfig {
        test_apps: vec![TestApp {
            name: "devine-app-integration-01-function".to_string(),
            role: ROLE_INTEGRATION.to_string(),
            ..TestApp::default()
        }],
        ..WorkflowConfig::default()
    };
    let error = workflow_scenarios(&args(&[]), &cfg).unwrap_err();
    assert!(error.contains("no environment has a base URL"), "{error}");
}

#[test]
fn positional_arguments_are_rejected_go_style() {
    let cfg = config(&McpApp::start(Vec::new()), &McpApp::start(Vec::new()));
    let error = workflow_scenarios(&args(&["stray", "args"]), &cfg).unwrap_err();
    assert_eq!(error, "scenarios: unexpected positional args [stray args]");
}

#[test]
fn a_result_serialises_in_declaration_order_and_drops_empty_details() {
    let result = ScenarioResult {
        app: "a".to_string(),
        env: "fat".to_string(),
        base: "http://x".to_string(),
        tool: "it_mcp_chain".to_string(),
        verdict: "PASS".to_string(),
        ..ScenarioResult::default()
    };
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        r#"{"app":"a","env":"fat","base":"http://x","tool":"it_mcp_chain","verdict":"PASS"}"#
    );
}

#[test]
fn only_pass_and_missing_count_as_not_failing() {
    let mut result = ScenarioResult::default();
    for (verdict, failing) in [
        ("PASS", false),
        ("MISSING", false),
        ("FAIL", true),
        ("ERROR", true),
        ("", true),
    ] {
        result.verdict = verdict.to_string();
        assert_eq!(result.is_failure(), failing, "{verdict:?}");
    }
}
