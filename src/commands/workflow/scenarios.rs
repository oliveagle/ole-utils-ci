// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `workflow scenarios`: run the per-app integration scenario MCP tools
//! across every environment.
//!
//! The integration apps publish their coverage as MCP tools, one tool per
//! scenario. Those tools address peers by short name, so one YAML proves both
//! FAT-to-FAT and PROD-to-PROD routing; only the workflow knows the whole
//! catalog, which is why this sweep lives here.
//!
//! Verdicts: `PASS` and `FAIL` come from the tool's own payload, `MISSING`
//! means the app does not ship that tool yet, and `ERROR` is a transport or
//! parse failure. Missing tools only fail the run with `--strict`.

use serde::Serialize;

use super::flags::{parse_with, Take, WorkflowFlags};
use super::model::{TestApp, WorkflowConfig, ROLE_INTEGRATION};
use super::scenario_call::run_scenario_tool;
use super::util::{go_slice, write_output_json};

/// The scenario set every integration app is expected to publish.
pub const SCENARIO_TOOLS: [&str; 4] = [
    "it_downstream_routing",
    "it_mcp_chain",
    "it_version_consistency",
    "it_health_checks",
];

/// The verdict of one (app, environment, tool) cell.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ScenarioResult {
    pub(super) app: String,
    pub(super) env: String,
    pub(super) base: String,
    pub(super) tool: String,
    pub(super) verdict: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) environment: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) self_idc: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) peer_idc: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) detail: String,
}

impl ScenarioResult {
    /// Whether the cell counts as a failure (anything but PASS or MISSING).
    fn is_failure(&self) -> bool {
        self.verdict != "PASS" && self.verdict != "MISSING"
    }
}

/// The flags of `workflow scenarios`.
#[derive(Debug, Clone)]
pub struct ScenarioFlags {
    common: WorkflowFlags,
    env: String,
    app: String,
    tool: String,
    strict: bool,
}

impl Default for ScenarioFlags {
    fn default() -> ScenarioFlags {
        ScenarioFlags {
            common: WorkflowFlags::default(),
            env: "all".to_string(),
            app: String::new(),
            tool: String::new(),
            strict: false,
        }
    }
}

fn parse_scenarios(args: &[String]) -> (ScenarioFlags, Vec<String>) {
    let mut flags = ScenarioFlags::default();
    let rest = parse_with(args, |arg, next| {
        if let Some(take) = flags.common.take_common(arg, next) {
            return Some(take);
        }
        match (arg, next) {
            ("--env", Some(value)) => {
                flags.env = value.to_lowercase();
                Some(Take::Value)
            }
            ("--app", Some(value)) => {
                flags.app = value.to_string();
                Some(Take::Value)
            }
            ("--tool", Some(value)) => {
                flags.tool = value.to_string();
                Some(Take::Value)
            }
            ("--strict", _) => {
                flags.strict = true;
                Some(Take::Rest)
            }
            _ => None,
        }
    });
    (flags, rest)
}

/// One (environment, app, base URL) target.
struct Target {
    env: &'static str,
    app: TestApp,
    base: String,
}

/// `workflow scenarios`.
pub fn workflow_scenarios(args: &[String], cfg: &WorkflowConfig) -> Result<i32, String> {
    let (flags, positional) = parse_scenarios(args);
    if !positional.is_empty() {
        return Err(format!(
            "scenarios: unexpected positional args {}",
            go_slice(&positional)
        ));
    }
    if !matches!(flags.env.as_str(), "all" | "fat" | "prod") {
        return Err(format!(
            "scenarios: --env must be fat|prod|all, got {:?}",
            flags.env
        ));
    }
    let apps = select_apps(cfg, &flags.app);
    if apps.is_empty() {
        return Err(format!(
            "scenarios: no integration app in catalog matches app={:?}",
            flags.app
        ));
    }
    let tools: Vec<String> = if flags.tool.is_empty() {
        SCENARIO_TOOLS.iter().map(|tool| tool.to_string()).collect()
    } else {
        vec![flags.tool.clone()]
    };
    let targets = build_targets(&apps, &flags.env);
    if targets.is_empty() {
        return Err("scenarios: no environment has a base URL for the selected apps".to_string());
    }
    if flags.common.dry_run {
        println!(
            "[workflow scenarios] dry-run: would run {} scenario(s) × {} target(s)",
            tools.len(),
            targets.len()
        );
        for target in &targets {
            for tool in &tools {
                println!(
                    "  [dry] {:<6} {:<38} {:<24} → POST {}/mcp",
                    target.env, target.app.name, tool, target.base
                );
            }
        }
        return Ok(0);
    }

    let mut results: Vec<ScenarioResult> = Vec::with_capacity(targets.len() * tools.len());
    for target in &targets {
        for tool in &tools {
            let result = run_scenario_tool(target.env, &target.app.name, &target.base, tool);
            if flags.common.verbose {
                println!(
                    "  [{}] {} {} → {}",
                    result.env, result.app, result.tool, result.verdict
                );
            }
            results.push(result);
        }
    }
    if flags.common.output == "json" {
        write_output_json(&results)?;
    } else {
        print_results(&results);
    }
    verdict(&results, flags.strict)
}

/// The integration apps, optionally narrowed to one name.
fn select_apps(cfg: &WorkflowConfig, app: &str) -> Vec<TestApp> {
    cfg.test_apps
        .iter()
        .filter(|candidate| candidate.role == ROLE_INTEGRATION)
        .filter(|candidate| app.is_empty() || candidate.name == app)
        .cloned()
        .collect()
}

/// The (app, environment) matrix that has a base URL.
fn build_targets(apps: &[TestApp], env: &str) -> Vec<Target> {
    let mut targets = Vec::new();
    for app in apps {
        if env == "all" || env == "fat" {
            if let Some(base) = scenario_base_url(app, "fat") {
                targets.push(Target {
                    env: "fat",
                    app: app.clone(),
                    base,
                });
            }
        }
        if env == "all" || env == "prod" {
            if let Some(base) = scenario_base_url(app, "prod") {
                targets.push(Target {
                    env: "prod",
                    app: app.clone(),
                    base,
                });
            }
        }
    }
    targets
}

/// FAT comes from `version_url`, PROD from `prod_url`; both lose the
/// `/version` suffix.
fn scenario_base_url(app: &TestApp, env: &str) -> Option<String> {
    let raw = match env {
        "fat" => app.version_url.as_str(),
        "prod" => app.prod_url.as_str(),
        _ => "",
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    Some(
        raw.trim_end_matches("/version")
            .trim_end_matches('/')
            .to_string(),
    )
}

/// The matrix rendering, one row per (app, environment, scenario).
fn print_results(results: &[ScenarioResult]) {
    println!(
        "{:<38} {:<6} {:<24} {:<9} detail",
        "app", "env", "scenario", "verdict"
    );
    for result in results {
        let mut detail = result.detail.clone();
        if result.verdict == "PASS" && !result.environment.is_empty() {
            detail = format!("env={}", result.environment);
            if !result.self_idc.is_empty() && !result.peer_idc.is_empty() {
                detail.push_str(&format!(
                    " self={} peer={}",
                    result.self_idc, result.peer_idc
                ));
            }
        }
        println!(
            "{:<38} {:<6} {:<24} {:<9} {}",
            result.app, result.env, result.tool, result.verdict, detail
        );
    }
}

/// Aggregates the verdicts into an exit code.
fn verdict(results: &[ScenarioResult], strict: bool) -> Result<i32, String> {
    let failures = results.iter().filter(|result| result.is_failure()).count();
    let missing = results
        .iter()
        .filter(|result| result.verdict == "MISSING")
        .count();
    if failures > 0 {
        return Err(format!("scenarios: {failures} scenario(s) did not pass"));
    }
    if missing > 0 {
        if strict {
            return Err(format!(
                "scenarios: {missing} scenario tool(s) missing (--strict)"
            ));
        }
        // stderr, so `--output json` leaves stdout a single JSON document.
        eprintln!(
            "\n! {missing} scenario tool(s) MISSING — app(s) not yet deployed with the scenario tools."
        );
        eprintln!("  Pass --strict to treat MISSING as a failure.");
    }
    Ok(0)
}

#[cfg(test)]
mod tests;
