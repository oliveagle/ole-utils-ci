// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The plan `deploy-prod` prints before it publishes: the app, the image and
//! every PROD group with its state.

use crate::commands::workflow::model::TestApp;

/// The tool that must still be exposed on PROD `/spec` after the deploy.
///
/// Precedence: `--spec-tool`, then the catalog entry, then the built-in
/// default for the app's origin (`ping` for a scaffolded FAAS app,
/// `http_self_ping` for a catalogued one, whose debug endpoints are open).
pub fn spec_tool_for(flag: &str, app: &TestApp, from_faas: bool) -> String {
    if !flag.is_empty() {
        return flag.to_string();
    }
    if !app.spec_tool.is_empty() {
        return app.spec_tool.clone();
    }
    if from_faas {
        "ping".to_string()
    } else {
        "http_self_ping".to_string()
    }
}

/// The header block: what is deployed where, and which groups are skipped.
pub fn plan_text(app: &TestApp, image: &str) -> String {
    let mut text = format!(
        "[workflow deploy-prod] app={} project={}\n  FAT group={} image={}\n  PROD groups: {} enabled / {} total\n",
        app.name,
        app.project_path,
        app.fat_group,
        image,
        app.enabled_groups(),
        app.prod_groups.len()
    );
    for group in &app.prod_groups {
        let state = if group.enabled { "enabled" } else { "DISABLED" };
        text.push_str(&format!(
            "    group={} name={} ({})\n",
            group.id, group.name, state
        ));
    }
    text
}

/// Prints the plan.
pub fn print_plan(app: &TestApp, image: &str) {
    print!("{}", plan_text(app, image));
}
