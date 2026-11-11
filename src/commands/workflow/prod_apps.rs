// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The selection rules `prod-verify` applies: which versions count as stable,
//! and which apps are verified, in which order.

use super::model::{TestApp, WorkflowConfig};

/// The order the apps are verified in: integration-02 (the clean-install
/// check) first, then integration-01 and the dev-test apps.
pub const PREFERRED_ORDER: [&str; 4] = [
    "devine-app-integration-02-function",
    "devine-app-integration-01-function",
    "devine-test-004-function",
    "devine-test-001-function",
];

/// Whether `value` is a plain stable `x.y.z` (no `-rc.N`, no other suffix,
/// every component all digits).
pub fn is_stable_version(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.contains('-') {
        return false;
    }
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts
        .iter()
        .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

/// The catalog apps in preferred order, narrowed by the `--apps` list.
pub fn select_apps(cfg: &WorkflowConfig, filter: &str) -> Vec<TestApp> {
    let allow: Option<Vec<String>> = if filter.trim().is_empty() {
        None
    } else {
        Some(
            filter
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
        )
    };
    let allowed = |name: &str| match &allow {
        Some(names) => names.iter().any(|candidate| candidate == name),
        None => true,
    };
    let mut out: Vec<TestApp> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for name in PREFERRED_ORDER {
        let Some(app) = cfg.test_apps.iter().find(|app| app.name == name) else {
            continue;
        };
        if seen.iter().any(|seen| seen == name) || !allowed(name) {
            continue;
        }
        out.push(app.clone());
        seen.push(name.to_string());
    }
    for app in &cfg.test_apps {
        if seen.iter().any(|seen| seen == &app.name) || !allowed(&app.name) {
            continue;
        }
        out.push(app.clone());
        seen.push(app.name.clone());
    }
    out
}

#[cfg(test)]
mod tests;
