// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The BUILT-IN FALLBACK topology.
//!
//! `workflow` has to work with no config file at all, so the binary ships the
//! devine-golang-mcp-app flow and its four test FAAS apps. The authoritative
//! copy for a given repo lives in the repo itself (`.devine/workflow.yaml`)
//! and may override this by name; keeping the two in sync is a review task,
//! not something the binary can enforce.

use super::model::{
    Artifactory, BinaryRepo, ProdGroup, TestApp, WorkflowConfig, DEFAULT_WORKFLOW_NAME,
    ROLE_DEV_TEST, ROLE_INTEGRATION,
};

/// The built-in fallback workflow.
pub fn default_workflow_config() -> WorkflowConfig {
    WorkflowConfig {
        name: DEFAULT_WORKFLOW_NAME.to_string(),
        binary_repo: BinaryRepo {
            name: "devine-golang-mcp-app".to_string(),
            project_path: "devine/devine-golang-mcp-app".to_string(),
            gitlab_base: "https://git.dev.sh.ctripcorp.com/devine/devine-golang-mcp-app"
                .to_string(),
            version_file: "VERSION".to_string(),
            changelog_file: "CHANGELOG.md".to_string(),
            mirror_files: Vec::new(),
            remote: "origin".to_string(),
        },
        artifactory: Artifactory {
            base_url: "http://artifactory.release.ctripcorp.com/artifactory".to_string(),
            uat_path: "trip-generic-binary-uat-shanghai/devine-golang-mcp-app/uat/linux-amd64"
                .to_string(),
            prod_path: "trip-generic-binary-prod-shanghai/devine-golang-mcp-app/prod/linux-amd64"
                .to_string(),
            min_size_b: 1_000_000,
        },
        test_apps: vec![
            dev_test_004(),
            dev_test_001(),
            integration_01(),
            integration_02(),
        ],
    }
}

/// `devine-test-004-function`: 开发测试, debug ON, the authoritative rc sensor.
fn dev_test_004() -> TestApp {
    TestApp {
        name: "devine-test-004-function".to_string(),
        project_path: "faas/devine-test-004-function".to_string(),
        role: ROLE_DEV_TEST.to_string(),
        description: "开发测试 (dev test). debug endpoints open. Authoritative signal for rc releases — /version reports app_version == rc after CI picks up the new binary from UAT artifactory.".to_string(),
        debug: true,
        polluted: false,
        app_id: "310011825".to_string(),
        version_url: "http://devine-test-004-function.fws.faas.qa.nt.ctripcorp.com/version".to_string(),
        fat_group: 1_050_478,
        prod_groups: vec![group(1_050_772, "SHAXY", true)],
        prod_url: "http://devine-test-004-function.faas.ctripcorp.com".to_string(),
        spec_tool: "http_self_ping".to_string(),
    }
}

/// `devine-test-001-function`: the public read-only MCP playground.
fn dev_test_001() -> TestApp {
    TestApp {
        name: "devine-test-001-function".to_string(),
        project_path: "faas/devine-test-001-function".to_string(),
        role: ROLE_DEV_TEST.to_string(),
        description: "公开只读 MCP 测试 (public read-only MCP playground). Fully public, no OAuth. debug ON. Carries the full read-only MCP tool surface (flt_poi_search_match, flight/airport/eterm read-only, etc). Same dev-test role as test4; default empty lookup still resolves to test4 (authoritative rc smoke).".to_string(),
        debug: true,
        polluted: false,
        app_id: "310013281".to_string(),
        version_url: "http://devine-test-001-function.fws.faas.qa.nt.ctripcorp.com/version".to_string(),
        fat_group: 1_053_904,
        prod_groups: vec![group(1_053_920, "SHAXY", true)],
        prod_url: "http://devine-test-001-function.faas.ctripcorp.com".to_string(),
        spec_tool: "http_self_ping".to_string(),
    }
}

/// `devine-app-integration-01-function`: the rc release gate app.
fn integration_01() -> TestApp {
    TestApp {
        name: "devine-app-integration-01-function".to_string(),
        project_path: "faas/devine-app-integration-01-function".to_string(),
        role: ROLE_INTEGRATION.to_string(),
        description: "集成测试 (integration test). debug endpoints open. May carry polluted QConfig keys from prior versions — useful for verifying backward compatibility, NOT authoritative for new builds.".to_string(),
        debug: true,
        polluted: true,
        app_id: "310012871".to_string(),
        version_url: "http://devine-app-integration-01-function.fws.faas.qa.nt.ctripcorp.com/version".to_string(),
        fat_group: 1_042_011,
        // SGP stays DISABLED until the public-cloud image sync lands.
        prod_groups: vec![
            group(1_053_332, "SHAXY", true),
            group(1_042_473, "SHA-ALI", true),
            group(1_053_330, "SGP", false),
        ],
        prod_url: "http://devine-app-integration-01-function.faas.ctripcorp.com".to_string(),
        spec_tool: "http_self_ping".to_string(),
    }
}

/// `devine-app-integration-02-function`: the clean-install, debug-OFF app.
fn integration_02() -> TestApp {
    TestApp {
        name: "devine-app-integration-02-function".to_string(),
        project_path: "faas/devine-app-integration-02-function".to_string(),
        role: ROLE_INTEGRATION.to_string(),
        description: "干净集成测试 (clean integration, stable release only). Debug OFF. No polluted config — closest thing to a clean install verification of a new binary against an empty QConfig. Publishes to SHAXY + SGP; SHA-ALI disabled until its public-cloud route is opened.".to_string(),
        debug: false,
        polluted: false,
        app_id: "310013220".to_string(),
        version_url: "http://devine-app-integration-02-function.fws.faas.qa.nt.ctripcorp.com/version".to_string(),
        fat_group: 1_042_845,
        prod_groups: vec![
            group(1_053_776, "SHAXY", true),
            group(1_054_162, "SHA-ALI", false),
            group(1_054_164, "SGP", true),
        ],
        prod_url: "http://devine-app-integration-02-function.faas.ctripcorp.com".to_string(),
        // integration-02 runs debug OFF: no http_self_ping, guard on hello_tengo.
        spec_tool: "hello_tengo".to_string(),
    }
}

fn group(id: i64, name: &str, enabled: bool) -> ProdGroup {
    ProdGroup {
        id,
        name: name.to_string(),
        enabled,
    }
}

#[cfg(test)]
mod tests;
