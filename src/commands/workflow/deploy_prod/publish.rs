// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The publish step: one Captain release per enabled PROD group.

use std::time::Duration;

use crate::commands::workflow::captain::{CaptainClient, RELEASE_POLL_STEP};
use crate::commands::workflow::model::{ProdGroup, TestApp};

use super::DeployProdFlags;

/// Publishes the image to every enabled PROD group of the app.
pub fn deploy_groups(
    client: &CaptainClient,
    app: &TestApp,
    image: &str,
    timeout: i64,
    flags: &DeployProdFlags,
) -> Result<(), String> {
    for group in &app.prod_groups {
        if !group.enabled {
            println!(
                "[workflow deploy-prod] skip DISABLED PROD group {} ({})",
                group.id, group.name
            );
            continue;
        }
        deploy_group(client, group, image, timeout, flags, RELEASE_POLL_STEP).map_err(|error| {
            format!(
                "deploy-prod: PROD group {} ({}): {error}",
                group.id, group.name
            )
        })?;
    }
    Ok(())
}

/// Publishes the image to one group: auto_deploy, wait, roll out, SUCCESS.
pub fn deploy_group(
    client: &CaptainClient,
    group: &ProdGroup,
    image: &str,
    timeout: i64,
    flags: &DeployProdFlags,
    step: Duration,
) -> Result<(), String> {
    if flags.common.dry_run {
        println!(
            "  [dry] captain auto_deploy group={} image={image} → wait → rollout → SUCCESS",
            group.id
        );
        return Ok(());
    }
    let release_id = client
        .auto_deploy(group.id, image)
        .map_err(|error| format!("auto_deploy: {error}"))?;
    println!(
        "  group={} release={release_id} image={image} — waiting (timeout {timeout}s)",
        group.id
    );
    let status =
        client.wait_release_by_id(group.id, release_id, timeout, flags.common.verbose, step)?;
    if status != "SUCCESS" {
        return Err(format!("release ended in {status} (want SUCCESS)"));
    }
    println!("  group={} release={release_id} SUCCESS", group.id);
    Ok(())
}
