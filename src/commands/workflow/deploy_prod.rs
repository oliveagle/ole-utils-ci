// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `workflow deploy-prod`: publish the latest main image to an app's Captain
//! PROD release groups, then guard against spec drift.
//!
//! The deployment is idempotent: `auto_deploy` is followed by a wait that
//! keys on the release id it returned, and a PENDING release is rolled out
//! automatically, so re-running converges on SUCCESS.

mod plan;
mod publish;
mod spec;

pub use plan::{print_plan, spec_tool_for};
pub use publish::deploy_groups;
pub use spec::verify_prod_spec;

use super::captain::{CaptainClient, CAPTAIN_BASE_URL};
use super::faas::{resolve_faas_app, FAAS_BASE_URL};
use super::flags::{parse_with, scan_int, Take, WorkflowFlags};
use super::model::{TestApp, WorkflowConfig};
use super::registry::Context;
use super::util::go_slice;

/// The flags of `workflow deploy-prod`.
#[derive(Debug, Clone)]
pub struct DeployProdFlags {
    common: WorkflowFlags,
    app: String,
    image: String,
    captain_token: String,
    timeout: i64,
    spec_tool: String,
}

impl Default for DeployProdFlags {
    fn default() -> DeployProdFlags {
        DeployProdFlags {
            common: WorkflowFlags::default(),
            app: "dev-test".to_string(),
            image: String::new(),
            captain_token: String::new(),
            timeout: 0,
            spec_tool: String::new(),
        }
    }
}

fn parse_deploy_prod(args: &[String]) -> (DeployProdFlags, Vec<String>) {
    let mut flags = DeployProdFlags::default();
    let rest = parse_with(args, |arg, next| {
        if let Some(take) = flags.common.take_common(arg, next) {
            return Some(take);
        }
        match (arg, next) {
            ("--app", Some(value)) => {
                flags.app = value.to_string();
                Some(Take::Value)
            }
            ("--image", Some(value)) => {
                flags.image = value.to_string();
                Some(Take::Value)
            }
            ("--captain-token", Some(value)) => {
                flags.captain_token = value.to_string();
                Some(Take::Value)
            }
            ("--timeout", Some(value)) => {
                flags.timeout = scan_int(value);
                Some(Take::Value)
            }
            ("--spec-tool", Some(value)) => {
                flags.spec_tool = value.to_string();
                Some(Take::Value)
            }
            _ => None,
        }
    });
    (flags, rest)
}

/// `workflow deploy-prod`.
pub fn workflow_deploy_prod(
    args: &[String],
    cfg: &WorkflowConfig,
    _ctx: &Context,
) -> Result<i32, String> {
    let env_token = std::env::var("CAPTAIN_USER_TOKEN").unwrap_or_default();
    deploy_prod_with(args, cfg, CAPTAIN_BASE_URL, FAAS_BASE_URL, &env_token)
}

/// The body of `deploy-prod`, with the two API roots injected so a test can
/// point them at a loopback stand-in instead of the production services.
fn deploy_prod_with(
    args: &[String],
    cfg: &WorkflowConfig,
    captain_base: &str,
    faas_base: &str,
    env_token: &str,
) -> Result<i32, String> {
    let (flags, positional) = parse_deploy_prod(args);
    if !positional.is_empty() {
        return Err(format!(
            "deploy-prod: unexpected positional args {}",
            go_slice(&positional)
        ));
    }
    let token = resolve_token(&flags, env_token);
    let (app, from_faas) = resolve_app(cfg, faas_base, &token, &flags)?;
    if app.fat_group == 0 {
        return Err(format!(
            "deploy-prod: app {:?} has no fat_group configured in the catalog",
            app.name
        ));
    }
    let client = CaptainClient::new(captain_base, &token);
    let image = resolve_image(&client, &app, &flags)?;
    let timeout = if flags.timeout <= 0 {
        900
    } else {
        flags.timeout
    };
    print_plan(&app, &image);
    deploy_groups(&client, &app, &image, timeout, &flags)?;

    let spec_tool = spec_tool_for(&flags.spec_tool, &app, from_faas);
    if !app.prod_url.is_empty() && !flags.common.dry_run {
        verify_prod_spec(&app.prod_url, &spec_tool)
            .map_err(|error| format!("deploy-prod: PROD spec verify: {error}"))?;
    }
    println!(
        "[workflow deploy-prod] OK — {} PROD groups deployed + spec verified.",
        app.name
    );
    Ok(0)
}

/// The token to authenticate with: the flag wins over the environment.
fn resolve_token(flags: &DeployProdFlags, env_token: &str) -> String {
    if flags.captain_token.is_empty() {
        env_token.to_string()
    } else {
        flags.captain_token.clone()
    }
}

/// The target app, from the catalog or from FAAS.
///
/// A catalogued app can be dry-run entirely offline with no token. Any other
/// app is resolved from FAAS, whose reads need auth — requiring the token
/// here turns a bare 401 into a clear message.
fn resolve_app(
    cfg: &WorkflowConfig,
    faas_base: &str,
    token: &str,
    flags: &DeployProdFlags,
) -> Result<(TestApp, bool), String> {
    let catalog = cfg.find_test_app(&flags.app).cloned();
    if token.is_empty() && (catalog.is_err() || !flags.common.dry_run) {
        return Err("deploy-prod: --captain-token or env CAPTAIN_USER_TOKEN required \
                    (Captain UI session x-user-token, or the faas credential store OAuth access_token)"
            .to_string());
    }
    match catalog {
        Ok(app) => Ok((app, false)),
        Err(_) => Ok((
            resolve_faas_app(faas_base, token, &flags.app)
                .map_err(|error| format!("deploy-prod: {error}"))?,
            true,
        )),
    }
}

/// The image to publish.
///
/// It must be the exact one, timestamp suffix included, because auto_deploy
/// resolves nothing by itself.
fn resolve_image(
    client: &CaptainClient,
    app: &TestApp,
    flags: &DeployProdFlags,
) -> Result<String, String> {
    let mut image = flags.image.clone();
    if image.is_empty() && !flags.common.dry_run {
        image = client.latest_fat_image(app.fat_group).map_err(|error| {
            format!(
                "deploy-prod: resolve image from FAT group {}: {error}",
                app.fat_group
            )
        })?;
    }
    if image.is_empty() {
        image = "<latest-FAT-image>".to_string();
    }
    Ok(image)
}

#[cfg(test)]
mod tests;
