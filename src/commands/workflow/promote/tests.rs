// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for `workflow promote`, including the rule that a blocked gate
//! leaves the repository untouched.

mod harness;

use std::path::Path;
use std::sync::atomic::Ordering;

use harness::{Fixture, Services};
use tempfile::TempDir;

use super::*;
use crate::commands::workflow::defaults::default_workflow_config;
use crate::commands::workflow::gate::GATE_APP_NAME;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

/// The built-in topology pointed at the fixture and the mock services.
fn config(fixture: &Fixture, services: &Services) -> WorkflowConfig {
    let mut cfg = default_workflow_config();
    cfg.binary_repo.name = "test-binary".to_string();
    cfg.binary_repo.project_path = "test/test-binary".to_string();
    cfg.artifactory.base_url = services.base.clone();
    cfg.artifactory.prod_path = "repo/prod".to_string();
    cfg.artifactory.min_size_b = 4;
    for app in &mut cfg.test_apps {
        if app.name == GATE_APP_NAME {
            app.version_url = format!("{}/version", services.base);
        }
    }
    let _ = &fixture.root;
    cfg
}

fn ctx(dir: &Path) -> Context {
    Context {
        cwd: dir.to_path_buf(),
        home: String::new(),
    }
}

#[test]
fn a_dry_run_prints_the_plan_and_touches_nothing() {
    let fixture = Fixture::new("0.1.34-rc.2");
    let services = Services::start("0.1.34-rc.2");
    let cfg = config(&fixture, &services);
    assert_eq!(
        workflow_promote(&args(&["--dry-run"]), &cfg, &ctx(&fixture.local)).unwrap(),
        0
    );
    assert_eq!(
        services.version_hits.load(Ordering::SeqCst),
        0,
        "no gate probe"
    );
    assert_eq!(fixture.tags(), "");
}

#[test]
fn a_gate_pass_promotes_and_publishes() {
    let fixture = Fixture::new("0.1.34-rc.2");
    let services = Services::start("0.1.34-rc.2");
    let cfg = config(&fixture, &services);
    let code = workflow_promote(
        &args(&["--timeout", "1", "--skip-post-check"]),
        &cfg,
        &ctx(&fixture.local),
    )
    .expect("a vetted rc promotes");
    assert_eq!(code, 0);
    assert_eq!(services.version_hits.load(Ordering::SeqCst), 1);
    assert_eq!(services.prod_hits.load(Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::read_to_string(fixture.local.join("VERSION")).unwrap(),
        "0.1.34\n"
    );
    assert_eq!(fixture.bare_version(), "0.1.34");
    assert_eq!(fixture.tags(), "v0.1.34");
}

#[test]
fn a_blocked_gate_leaves_the_repository_untouched() {
    let fixture = Fixture::new("0.1.34-rc.2");
    let services = Services::start("0.1.33");
    let cfg = config(&fixture, &services);
    let error = workflow_promote(
        &args(&["--timeout", "1", "--skip-post-check"]),
        &cfg,
        &ctx(&fixture.local),
    )
    .unwrap_err();
    assert!(error.contains("release gate: BLOCKED"), "{error}");
    assert_eq!(services.prod_hits.load(Ordering::SeqCst), 0, "no publish");
    assert_eq!(
        std::fs::read_to_string(fixture.local.join("VERSION")).unwrap(),
        "0.1.34-rc.2\n"
    );
    assert_eq!(fixture.tags(), "", "no tag was created");
    assert_eq!(fixture.bare_version(), "0.1.34-rc.2");
}

#[test]
fn a_stable_version_has_nothing_to_promote() {
    let fixture = Fixture::new("0.1.34");
    let services = Services::start("0.1.34");
    let cfg = config(&fixture, &services);
    let error = workflow_promote(&args(&[]), &cfg, &ctx(&fixture.local)).unwrap_err();
    assert!(
        error.contains("already stable — nothing to promote"),
        "{error}"
    );
}

#[test]
fn the_from_flag_overrides_the_version_the_gate_checks() {
    let fixture = Fixture::new("0.1.34-rc.2");
    let services = Services::start("0.1.34-rc.9");
    let cfg = config(&fixture, &services);
    let code = workflow_promote(
        &args(&[
            "--timeout",
            "1",
            "--skip-post-check",
            "--from",
            "0.1.34-rc.9",
        ]),
        &cfg,
        &ctx(&fixture.local),
    )
    .expect("the gate checks the version named by --from");
    assert_eq!(code, 0);
}

#[test]
fn promote_outside_the_binary_repo_is_refused() {
    let dir = TempDir::new().unwrap();
    let services = Services::start("1.0.0");
    let cfg = config(&Fixture::new("0.1.34-rc.2"), &services);
    let error = workflow_promote(&args(&[]), &cfg, &ctx(dir.path())).unwrap_err();
    assert!(error.contains("cannot locate the binary repo"), "{error}");
}

#[test]
fn promote_rejects_positional_arguments() {
    let dir = TempDir::new().unwrap();
    let services = Services::start("1.0.0");
    let cfg = config(&Fixture::new("0.1.34-rc.2"), &services);
    let error = workflow_promote(&args(&["stray"]), &cfg, &ctx(dir.path())).unwrap_err();
    assert_eq!(error, "promote: unexpected positional args [stray]");
}

#[test]
fn the_parser_collects_every_documented_flag() {
    let (flags, rest) = parse_promote(&args(&[
        "--timeout",
        "30",
        "--from",
        "1.2.3-rc.1",
        "--poll-timeout",
        "60",
        "--skip-post-check",
        "--no-auto-checkout",
    ]));
    assert_eq!(flags.timeout, 30);
    assert_eq!(flags.from, "1.2.3-rc.1");
    assert_eq!(flags.poll_timeout, 60);
    assert!(flags.skip_post_check);
    assert!(!flags.auto_checkout);
    assert!(rest.is_empty());
}
