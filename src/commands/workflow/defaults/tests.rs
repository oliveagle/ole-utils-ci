// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests pinning the built-in fallback topology.
//!
//! It is what makes `workflow` usable with no config file, and `workflow apps`
//! prints it verbatim, so every value here is part of the observable surface.

use super::*;

/// The catalog entry for `name`.
fn app<'a>(cfg: &'a WorkflowConfig, name: &str) -> &'a TestApp {
    cfg.test_apps
        .iter()
        .find(|app| app.name == name)
        .unwrap_or_else(|| panic!("{name} missing from the built-in catalog"))
}

/// The enabled flag of one PROD group.
fn enabled(cfg: &WorkflowConfig, name: &str, group: i64) -> bool {
    app(cfg, name)
        .prod_groups
        .iter()
        .find(|candidate| candidate.id == group)
        .unwrap_or_else(|| panic!("group {group} missing from {name}"))
        .enabled
}

#[test]
fn builtin_is_the_mcp_app_with_its_repo_and_artifactory_paths() {
    let cfg = default_workflow_config();
    assert_eq!(cfg.name, DEFAULT_WORKFLOW_NAME);
    assert_eq!(cfg.binary_repo.name, "devine-golang-mcp-app");
    assert_eq!(cfg.binary_repo.project_path, "devine/devine-golang-mcp-app");
    assert_eq!(cfg.binary_repo.version_file, "VERSION");
    assert_eq!(cfg.binary_repo.changelog_file, "CHANGELOG.md");
    assert_eq!(cfg.binary_repo.remote, "origin");
    assert!(cfg.binary_repo.mirror_files.is_empty());
    assert_eq!(
        cfg.artifactory.base_url,
        "http://artifactory.release.ctripcorp.com/artifactory"
    );
    assert_eq!(
        cfg.artifactory.uat_path,
        "trip-generic-binary-uat-shanghai/devine-golang-mcp-app/uat/linux-amd64"
    );
    assert_eq!(
        cfg.artifactory.prod_path,
        "trip-generic-binary-prod-shanghai/devine-golang-mcp-app/prod/linux-amd64"
    );
    assert_eq!(cfg.artifactory.min_size_b, 1_000_000);
}

#[test]
fn catalog_holds_the_four_agreed_apps_in_order() {
    let cfg = default_workflow_config();
    let names: Vec<&str> = cfg.test_apps.iter().map(|app| app.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "devine-test-004-function",
            "devine-test-001-function",
            "devine-app-integration-01-function",
            "devine-app-integration-02-function",
        ]
    );
}

#[test]
fn dev_test_apps_are_debug_on_and_authoritative() {
    let cfg = default_workflow_config();
    for name in ["devine-test-004-function", "devine-test-001-function"] {
        let entry = app(&cfg, name);
        assert_eq!(entry.role, ROLE_DEV_TEST, "{name}");
        assert!(entry.debug, "{name} has debug on");
        assert!(!entry.polluted, "{name} is clean");
        assert_eq!(entry.spec_tool, "http_self_ping", "{name}");
    }
    assert_eq!(app(&cfg, "devine-test-004-function").app_id, "310011825");
    assert_eq!(app(&cfg, "devine-test-001-function").app_id, "310013281");
}

#[test]
fn integration_apps_carry_the_documented_debug_and_config_state() {
    let cfg = default_workflow_config();
    let dirty = app(&cfg, "devine-app-integration-01-function");
    assert_eq!(dirty.role, ROLE_INTEGRATION);
    assert!(dirty.debug, "integration-01 keeps debug on");
    assert!(dirty.polluted, "integration-01 may carry dirty QConfig");
    assert_eq!(dirty.spec_tool, "http_self_ping");

    let clean = app(&cfg, "devine-app-integration-02-function");
    assert_eq!(clean.role, ROLE_INTEGRATION);
    assert!(!clean.debug, "integration-02 runs debug off");
    assert!(!clean.polluted);
    assert_eq!(clean.spec_tool, "hello_tengo");
}

#[test]
fn integration01_publishes_to_shaxy_and_shali_with_sgp_suspended() {
    let cfg = default_workflow_config();
    let name = "devine-app-integration-01-function";
    assert_eq!(app(&cfg, name).fat_group, 1_042_011);
    assert!(enabled(&cfg, name, 1_053_332), "SHAXY");
    assert!(enabled(&cfg, name, 1_042_473), "SHA-ALI");
    assert!(!enabled(&cfg, name, 1_053_330), "SGP is suspended");
    assert_eq!(app(&cfg, name).enabled_groups(), 2);
}

#[test]
fn integration02_publishes_to_shaxy_and_sgp_with_shali_suspended() {
    let cfg = default_workflow_config();
    let name = "devine-app-integration-02-function";
    assert_eq!(app(&cfg, name).fat_group, 1_042_845);
    assert!(enabled(&cfg, name, 1_053_776), "SHAXY");
    assert!(!enabled(&cfg, name, 1_054_162), "SHA-ALI route closed");
    assert!(enabled(&cfg, name, 1_054_164), "SGP");
    assert_eq!(app(&cfg, name).enabled_groups(), 2);
}

#[test]
fn every_app_has_a_version_endpoint_and_a_prod_url() {
    let cfg = default_workflow_config();
    for entry in &cfg.test_apps {
        assert!(
            entry.version_url.ends_with("/version"),
            "{}: {}",
            entry.name,
            entry.version_url
        );
        assert!(!entry.prod_url.is_empty(), "{}", entry.name);
        assert_eq!(entry.project_path, format!("faas/{}", entry.name));
        assert!(!entry.description.is_empty(), "{}", entry.name);
    }
}
