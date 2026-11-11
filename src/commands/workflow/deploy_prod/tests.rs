// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for `workflow deploy-prod`: the token rule, the offline
//! dry-run, FAAS-resolved targets, the spec guard and the Captain handshake.

mod mock;

use std::time::Duration;

use mock::Mock;

use super::plan::plan_text;
use super::publish::deploy_group;
use super::*;
use crate::commands::workflow::defaults::default_workflow_config;
use crate::commands::workflow::model::ProdGroup;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

/// A cluster that refuses every connection.
const DEAD: &str = "http://127.0.0.1:1";

fn flags(dry_run: bool) -> DeployProdFlags {
    DeployProdFlags {
        common: WorkflowFlags {
            dry_run,
            ..WorkflowFlags::default()
        },
        ..DeployProdFlags::default()
    }
}

#[test]
fn the_plan_marks_every_group_and_the_suspended_one() {
    let cfg = default_workflow_config();
    let app = cfg.find_test_app("integration").unwrap();
    let text = plan_text(app, "gitlab-ci/1:main-abc");
    assert!(
        text.contains("app=devine-app-integration-01-function"),
        "{text}"
    );
    assert!(
        text.contains("FAT group=1042011 image=gitlab-ci/1:main-abc"),
        "{text}"
    );
    assert!(text.contains("PROD groups: 2 enabled / 3 total"), "{text}");
    assert!(
        text.contains("group=1053332 name=SHAXY (enabled)"),
        "{text}"
    );
    assert!(
        text.contains("group=1042473 name=SHA-ALI (enabled)"),
        "{text}"
    );
    assert!(text.contains("group=1053330 name=SGP (DISABLED)"), "{text}");
}

#[test]
fn a_catalogued_app_dry_runs_with_no_token_and_no_network() {
    let cfg = default_workflow_config();
    let code = deploy_prod_with(
        &args(&["--app", "devine-app-integration-01-function", "--dry-run"]),
        &cfg,
        DEAD,
        DEAD,
        "",
    )
    .expect("a catalogued dry-run must not need a credential");
    assert_eq!(code, 0);
}

#[test]
fn a_non_catalogued_dry_run_still_needs_a_token() {
    let cfg = default_workflow_config();
    let error = deploy_prod_with(
        &args(&["--app", "faas/agentark-data-admin-function", "--dry-run"]),
        &cfg,
        DEAD,
        DEAD,
        "",
    )
    .unwrap_err();
    assert!(error.contains("CAPTAIN_USER_TOKEN"), "{error}");
    assert!(!error.contains("HTTP 401"), "must stay actionable: {error}");
}

#[test]
fn a_catalogued_publish_needs_a_token() {
    let cfg = default_workflow_config();
    let error = deploy_prod_with(
        &args(&["--app", "devine-app-integration-01-function"]),
        &cfg,
        DEAD,
        DEAD,
        "",
    )
    .unwrap_err();
    assert!(error.contains("CAPTAIN_USER_TOKEN"), "{error}");
}

#[test]
fn a_faas_resolved_app_reports_release_groups_not_ksvc_ids() {
    let function = r#"{"count":1,"result":[{"id":310013516,"name":"agentark-data-admin-function",
        "language":"golang","repo":"faas/agentark-data-admin-function","importance_level":"L5",
        "pipeline_template":{"pipeline_code":"GolangDevineMCPFunc"}}]}"#;
    let ksvcs = r#"{"agentark-data-admin-function":[
        {"id":27417,"env":"fat","region":"SHAXY","idc":"SHAXY","url":"http://x.example","group_id":1043671},
        {"id":27819,"env":"prod","region":"SHAXY","idc":"SHAXY","url":"https://x.example","group_id":1044275},
        {"id":27820,"env":"prod","region":"SHARB","idc":"SHARB","url":"https://x.example","group_id":1044277}]}"#;
    let server = Mock::start(move |seen| {
        if seen.path.starts_with("/functions/") {
            (200, ksvcs.to_string())
        } else {
            (200, function.to_string())
        }
    });
    let cfg = default_workflow_config();
    let code = deploy_prod_with(
        &args(&[
            "--app",
            "faas/agentark-data-admin-function",
            "--dry-run",
            "--captain-token",
            "tok-abc",
        ]),
        &cfg,
        DEAD,
        &server.base,
        "",
    )
    .expect("a FAAS-resolved dry-run");
    assert_eq!(code, 0);
    assert_eq!(
        server.paths(),
        [
            "/functions?repo=faas/agentark-data-admin-function".to_string(),
            "/functions/310013516/ksvcs".to_string(),
        ]
    );
    let request = &server.requests()[0];
    assert!(request.header("Authorization").starts_with("Bearer "));
    assert!(!request.header("X-User-Token").is_empty());

    // The resolved topology is what the plan prints.
    let app =
        resolve_faas_app(&server.base, "tok-abc", "faas/agentark-data-admin-function").unwrap();
    let text = plan_text(&app, "<latest-FAT-image>");
    for want in ["1043671", "1044275", "1044277"] {
        assert!(text.contains(want), "{want} missing from:\n{text}");
    }
}

#[test]
fn the_spec_tool_precedence_is_flag_catalog_default() {
    let mut app = default_workflow_config()
        .find_test_app("devine-app-integration-02-function")
        .cloned()
        .unwrap();
    assert_eq!(spec_tool_for("", &app, false), "hello_tengo");
    assert_eq!(spec_tool_for("--x", &app, false), "--x");
    app.spec_tool.clear();
    assert_eq!(spec_tool_for("", &app, true), "ping");
    assert_eq!(spec_tool_for("", &app, false), "http_self_ping");
}

#[test]
fn the_spec_guard_accepts_a_tool_that_is_present() {
    let server = Mock::start(|_| {
        (
            200,
            r#"[{"name":"hello_tengo"},{"name":"http_self_ping"}]"#.to_string(),
        )
    });
    verify_prod_spec(&server.base, "hello_tengo").expect("tool present");
    assert_eq!(server.paths(), ["/spec"]);
}

#[test]
fn the_spec_guard_reports_drift_with_the_tool_count() {
    let server = Mock::start(|_| (200, r#"[{"name":"other"}]"#.to_string()));
    let error = verify_prod_spec(&server.base, "hello_tengo").unwrap_err();
    assert!(error.contains("does not expose \"hello_tengo\""), "{error}");
    assert!(error.contains("(1 tools)"), "{error}");
}

#[test]
fn the_spec_guard_reports_unparseable_and_failed_answers() {
    let garbage = Mock::start(|_| (200, "not json".to_string()));
    let error = verify_prod_spec(&garbage.base, "x").unwrap_err();
    assert!(error.contains("parse "), "{error}");
    assert!(error.contains("body: not json"), "{error}");

    // Go parses the body even on a non-2xx answer, so a dead host is the
    // path that exercises the transport error.
    let error = verify_prod_spec("http://127.0.0.1:1", "x").unwrap_err();
    assert!(
        error.starts_with("GET http://127.0.0.1:1/spec: "),
        "{error}"
    );
}

#[test]
fn a_dry_run_group_prints_the_sequence_and_sends_nothing() {
    let server = Mock::start(|_| (500, String::new()));
    let client = CaptainClient::new(&server.base, "tok");
    let group = ProdGroup {
        id: 7,
        name: "SHAXY".to_string(),
        enabled: true,
    };
    deploy_group(
        &client,
        &group,
        "img",
        5,
        &flags(true),
        Duration::from_millis(1),
    )
    .expect("dry run");
    assert!(server.requests().is_empty());
}

#[test]
fn a_real_group_publishes_waits_and_confirms() {
    let rolled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = std::sync::Arc::clone(&rolled);
    let server = Mock::start(move |seen| {
        if seen.path == "/releases/auto_deploy" {
            return (200, r#"{"release":{"id":77}}"#.to_string());
        }
        if seen.path == "/releases/77/rollout" {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            return (200, "{}".to_string());
        }
        let status = if flag.load(std::sync::atomic::Ordering::SeqCst) {
            "SUCCESS"
        } else {
            "PENDING"
        };
        (
            200,
            format!(r#"{{"result":[{{"id":77,"status":"{status}"}}]}}"#),
        )
    });
    let client = CaptainClient::new(&server.base, "tok");
    let group = ProdGroup {
        id: 7,
        name: "SHAXY".to_string(),
        enabled: true,
    };
    deploy_group(
        &client,
        &group,
        "img",
        30,
        &flags(false),
        Duration::from_millis(1),
    )
    .expect("the release converges on SUCCESS");
    let auto = &server.requests()[0];
    assert_eq!(auto.method, "POST");
    assert_eq!(auto.path, "/releases/auto_deploy");
    assert!(auto.body.contains("\"id\":7"), "{}", auto.body);
}

#[test]
fn an_app_without_a_fat_group_cannot_be_published() {
    let mut cfg = default_workflow_config();
    cfg.test_apps[0].fat_group = 0;
    let error = deploy_prod_with(
        &args(&["--app", "devine-test-004-function", "--captain-token", "t"]),
        &cfg,
        DEAD,
        DEAD,
        "",
    )
    .unwrap_err();
    assert!(error.contains("has no fat_group configured"), "{error}");
}

#[test]
fn positional_arguments_are_rejected_go_style() {
    let cfg = default_workflow_config();
    let error = deploy_prod_with(&args(&["stray"]), &cfg, DEAD, DEAD, "").unwrap_err();
    assert_eq!(error, "deploy-prod: unexpected positional args [stray]");
}
