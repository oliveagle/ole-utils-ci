// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `workflow` command: the devine binary's release and test flow, as one
//! set of named commands.
//!
//! The flow it codifies:
//!
//! 1. release a new rc of the binary runtime (bump on main, push main + tag,
//!    CI uploads UAT — an rc never publishes PROD);
//! 2. smoke-test the rc on the dev-test app, whose `/version` endpoint is the
//!    authoritative signal that the new binary is live;
//! 3. promote the vetted rc to stable, gated on integration-01 having
//!    verified it, and re-tested on integration-02 immediately after;
//! 4. exercise the integration apps in a production-like setting;
//! 5. publish the main image to Captain PROD groups and verify the PROD spec;
//! 6. sweep the per-app integration scenarios over FAT and PROD.
//!
//! The topology is DATA that follows the repo's commits, not code frozen in
//! the binary: every workflow has a name, a repo-committed
//! `.devine/workflow.yaml` is the source of truth, and the binary ships a
//! built-in fallback (`devine-golang-mcp-app`) so the command works with no
//! config file at all. A named workflow is driven as
//! `workflow <name> <subcommand>`; the bare form uses the default workflow.

mod apps_cmd;
mod bump;
mod captain;
mod defaults;
mod deploy_prod;
mod faas;
mod flags;
mod gate;
mod http;
mod model;
mod prod_apps;
mod prod_verify;
mod promote;
mod registry;
mod release;
mod scenario_call;
mod scenarios;
mod status;
mod usage;
mod usage_general;
mod usage_subs;
mod util;

use model::{WorkflowConfig, DEFAULT_WORKFLOW_NAME};
use registry::{Context, Registry};
use util::non_empty_or;

/// The subcommands a named workflow accepts, in usage order.
pub const SUBCOMMANDS: [&str; 9] = [
    "apps",
    "release-rc",
    "test-rc",
    "integration-rc",
    "deploy-prod",
    "promote",
    "status",
    "scenarios",
    "prod-verify",
];

/// The `devine-utils-ci workflow` entry point.
pub fn workflow(args: &[String]) -> Result<i32, String> {
    if args.is_empty() {
        usage::print_usage(&mut std::io::stderr(), "");
        return Err(
            "workflow: subcommand required (try `workflow list` or `workflow <name> <subcommand>`)"
                .to_string(),
        );
    }
    let ctx = Context::current();
    let registry = registry::load_layered(&registry::extract_config_flag(args), &ctx)
        .map_err(|error| format!("workflow: load config: {error}"))?;
    dispatch(&registry, args, &ctx)
}

/// Resolves `args` against the registry and runs the requested subcommand.
fn dispatch(reg: &Registry, args: &[String], ctx: &Context) -> Result<i32, String> {
    let first = args[0].as_str();
    match first {
        "-h" | "--help" | "help" => {
            usage::print_usage(&mut std::io::stdout(), "");
            return Ok(0);
        }
        "list" => return apps_cmd::workflow_list(&args[1..], reg),
        _ => {}
    }
    if let Some(cfg) = reg.get(first) {
        if args.len() < 2 {
            return Err(format!(
                "workflow {first}: subcommand required ({})",
                SUBCOMMANDS.join("|")
            ));
        }
        return dispatch_one(cfg, &args[1], &args[2..], ctx);
    }
    // Neither a workflow name nor a subcommand: report both possibilities.
    if !is_subcommand(first) {
        usage::print_usage(&mut std::io::stderr(), first);
        return Err(format!(
            "workflow: unknown workflow or subcommand {first:?} (run `workflow list` to see the named workflows)"
        ));
    }
    match reg.default_workflow() {
        Some(cfg) => dispatch_one(cfg, first, &args[1..], ctx),
        None => Err("workflow: nil workflow config".to_string()),
    }
}

/// Whether `name` is one of the workflow subcommands.
fn is_subcommand(name: &str) -> bool {
    SUBCOMMANDS.contains(&name)
}

/// Runs `sub` against one workflow's topology.
fn dispatch_one(
    cfg: &WorkflowConfig,
    sub: &str,
    rest: &[String],
    ctx: &Context,
) -> Result<i32, String> {
    match sub {
        "apps" => apps_cmd::workflow_apps(rest, cfg),
        "release-rc" => release::workflow_release_rc(rest, cfg, ctx),
        "test-rc" => apps_cmd::workflow_test_rc(rest, cfg, ctx),
        "integration-rc" => apps_cmd::workflow_integration_rc(rest, cfg, ctx),
        "deploy-prod" => deploy_prod::workflow_deploy_prod(rest, cfg, ctx),
        "promote" => promote::workflow_promote(rest, cfg, ctx),
        "status" => status::workflow_status(rest, cfg, ctx),
        "scenarios" => scenarios::workflow_scenarios(rest, cfg),
        "prod-verify" => prod_verify::workflow_prod_verify(rest, cfg, ctx),
        "list" => Err(format!(
            "workflow {}: {}",
            non_empty_or(&cfg.name, DEFAULT_WORKFLOW_NAME),
            apps_cmd::LIST_HINT
        )),
        "-h" | "--help" | "help" => {
            usage::print_usage(&mut std::io::stdout(), "");
            Ok(0)
        }
        other => {
            usage::print_usage(&mut std::io::stderr(), other);
            Err(format!(
                "workflow {}: unknown subcommand {other:?} (valid: {})",
                non_empty_or(&cfg.name, DEFAULT_WORKFLOW_NAME),
                SUBCOMMANDS.join("|")
            ))
        }
    }
}

#[cfg(test)]
mod tests;
