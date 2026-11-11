// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the `workflow` entry point: the subcommand inventory and the
//! resolution rules between a named workflow and the built-in default.

use super::*;
use crate::commands::workflow::defaults::default_workflow_config;
use crate::commands::workflow::model::BinaryRepo;
use crate::commands::workflow::model::TestApp;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

/// A context that is not inside any repository and has no home config.
fn ctx() -> Context {
    Context {
        cwd: std::path::PathBuf::from("/nonexistent"),
        home: String::new(),
    }
}

/// The built-in registry plus one user workflow.
fn registry() -> Registry {
    let mut reg = Registry::builtin();
    reg.insert(WorkflowConfig {
        name: "my-app".to_string(),
        binary_repo: BinaryRepo {
            name: "my-app".to_string(),
            project_path: "ns/my-app".to_string(),
            version_file: "VERSION".to_string(),
            ..Default::default()
        },
        test_apps: vec![TestApp {
            name: "my-dev".to_string(),
            project_path: "faas/my-dev".to_string(),
            role: "dev-test".to_string(),
            version_url: "http://x/version".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    });
    reg
}

#[test]
fn the_subcommand_inventory_is_the_documented_order() {
    assert_eq!(
        SUBCOMMANDS,
        [
            "apps",
            "release-rc",
            "test-rc",
            "integration-rc",
            "deploy-prod",
            "promote",
            "status",
            "scenarios",
            "prod-verify",
        ]
    );
    assert!(is_subcommand("scenarios"));
    assert!(is_subcommand("prod-verify"));
    assert!(!is_subcommand("list"));
    assert!(!is_subcommand("nope"));
}

#[test]
fn list_renders_the_registry() {
    assert_eq!(dispatch(&registry(), &args(&["list"]), &ctx()).unwrap(), 0);
}

#[test]
fn a_named_workflow_reaches_its_own_subcommand() {
    assert_eq!(
        dispatch(&registry(), &args(&["my-app", "apps"]), &ctx()).unwrap(),
        0
    );
    // The bare form resolves the built-in default.
    assert_eq!(dispatch(&registry(), &args(&["apps"]), &ctx()).unwrap(), 0);
}

#[test]
fn a_named_workflow_without_a_subcommand_says_which_ones_exist() {
    let error = dispatch(&registry(), &args(&["my-app"]), &ctx()).unwrap_err();
    assert_eq!(
        error,
        format!(
            "workflow my-app: subcommand required ({})",
            SUBCOMMANDS.join("|")
        )
    );
}

#[test]
fn list_inside_a_named_workflow_points_at_the_top_level_command() {
    let error = dispatch(&registry(), &args(&["my-app", "list"]), &ctx()).unwrap_err();
    assert!(
        error.starts_with("workflow my-app: `list` is a top-level command"),
        "{error}"
    );
    assert!(error.contains("devine-utils-ci workflow list"), "{error}");
}

#[test]
fn an_unknown_subcommand_of_a_named_workflow_lists_the_valid_ones() {
    let error = dispatch(&registry(), &args(&["my-app", "bogus"]), &ctx()).unwrap_err();
    assert_eq!(
        error,
        format!(
            "workflow my-app: unknown subcommand \"bogus\" (valid: {})",
            SUBCOMMANDS.join("|")
        )
    );
}

#[test]
fn an_unknown_first_token_reports_both_possibilities() {
    let error = dispatch(&registry(), &args(&["no-such-workflow", "apps"]), &ctx()).unwrap_err();
    assert!(
        error.contains("unknown workflow or subcommand \"no-such-workflow\""),
        "{error}"
    );
    assert!(error.contains("workflow list"), "{error}");
}

#[test]
fn help_is_a_success_on_stdout() {
    for spelling in ["-h", "--help", "help"] {
        assert_eq!(
            dispatch(&registry(), &args(&[spelling]), &ctx()).unwrap(),
            0
        );
    }
    // Inside a named workflow too.
    assert_eq!(
        dispatch(&registry(), &args(&["my-app", "-h"]), &ctx()).unwrap(),
        0
    );
}

#[test]
fn a_bare_invocation_needs_a_subcommand() {
    let error = workflow(&[]).unwrap_err();
    assert!(error.contains("workflow: subcommand required"), "{error}");
    assert!(error.contains("workflow list"), "{error}");
}

#[test]
fn a_registry_without_the_builtin_cannot_resolve_the_bare_form() {
    let mut reg = registry();
    reg.insert(WorkflowConfig {
        name: DEFAULT_WORKFLOW_NAME.to_string(),
        ..WorkflowConfig::default()
    });
    let empty = Registry::default();
    assert!(empty.default_workflow().is_none());
    // `Registry::default()` has no built-in at all: the bare form must say so.
    let error = dispatch(&empty, &args(&["apps"]), &ctx()).unwrap_err();
    assert_eq!(error, "workflow: nil workflow config");
    let _ = default_workflow_config();
}

#[test]
fn the_registry_helpers_report_what_they_hold() {
    let reg = registry();
    let names: Vec<&str> = reg.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, [DEFAULT_WORKFLOW_NAME, "my-app"]);
    assert_eq!(
        reg.default_workflow().map(|cfg| cfg.name.clone()),
        Some(DEFAULT_WORKFLOW_NAME.to_string())
    );
}
