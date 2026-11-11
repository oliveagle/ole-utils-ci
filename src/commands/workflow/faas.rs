// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Resolving a `deploy-prod` target that is NOT in the workflow catalog,
//! straight from the FAAS function registry.
//!
//! Two hard restrictions are checked before any write, both fail-closed:
//!
//! 1. only FAAS **devine golang** apps — `pipeline_template.pipeline_code`
//!    must carry the `GolangDevine` prefix and the language must be golang;
//! 2. only **non-core** apps — `importance_level` must be L4 or L5. L1/L2/L3
//!    touch the main flow, and an absent or unknown level is refused.

use std::time::Duration;

use serde::Deserialize;

use crate::httpd;

use super::captain::CAPTAIN_BASE_URL;
use super::model::{ProdGroup, TestApp};

/// The pipeline template prefix that identifies a devine-golang FAAS app.
pub const DEVINE_GOLANG_PIPELINE_PREFIX: &str = "GolangDevine";

/// One FAAS function record.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FaasFunction {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub importance_level: String,
    #[serde(default)]
    pub pipeline_template: PipelineTemplate,
}

/// The template a function was scaffolded from.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PipelineTemplate {
    #[serde(default, rename = "pipeline_code")]
    pub code: String,
}

/// One FAAS ksvc (one deployment environment of a function).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FaasKsvc {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub env: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub idc: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub deleted_at: Option<String>,
    /// The Captain release-group id, which the release API keys on. The ksvc
    /// id above is the service-level id and returns an empty release list.
    #[serde(default)]
    pub group_id: i64,
}

/// Normalises an `--app` value into the repo path FAAS filters on.
///
/// `faas/foo-function` stays as-is; `foo-function` and `foo` both become
/// `faas/foo-function`, because FAAS scaffolds every function repo that way.
pub fn faas_repo_path(app_name: &str) -> String {
    let name = app_name.trim().trim_end_matches(".git");
    if name.contains('/') {
        return name.to_string();
    }
    let name = name.trim_end_matches("-function");
    format!("faas/{name}-function")
}

/// One read-only FAAS API GET.
fn faas_request(base: &str, token: &str, path: &str) -> Result<Vec<u8>, String> {
    let url = format!("{base}{path}");
    let mut headers: Vec<(String, String)> =
        vec![("Accept".to_string(), "application/json".to_string())];
    if !token.is_empty() {
        headers.push(("Authorization".to_string(), format!("Bearer {token}")));
        headers.push(("X-User-Token".to_string(), token.to_string()));
    }
    let borrowed: Vec<(&str, &str)> = headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let response = httpd::request("GET", &url, &borrowed, None, Duration::from_secs(30))
        .map_err(|error| format!("FAAS GET {path} → {error}"))?;
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "FAAS GET {path} → HTTP {}: {}",
            response.status,
            super::util::first_chars(&response.body_string(), 300)
        ));
    }
    Ok(response.body)
}

/// Resolves an app that is not in the catalog as a FAAS function.
///
/// Returns a synthetic [`TestApp`]: the FAT ksvc is the image source and the
/// PROD ksvcs become the release groups.
pub fn resolve_faas_app(base: &str, token: &str, app_name: &str) -> Result<TestApp, String> {
    let repo = faas_repo_path(app_name);
    let raw = faas_request(base, token, &format!("/functions?repo={repo}"))
        .map_err(|error| format!("look up FAAS function {repo:?}: {error}"))?;
    let list: FunctionList = serde_json::from_slice(&raw)
        .map_err(|error| format!("parse FAAS function list: {error}"))?;
    let Some(function) = list.result.into_iter().next() else {
        return Err(format!(
            "no FAAS function for repo {repo:?} (not in the catalog either; \
             pass --app <catalog-name|role> or a FAAS repo path)"
        ));
    };
    check_devine_golang(&function)?;
    check_non_core(&function)?;

    let raw = faas_request(base, token, &format!("/functions/{}/ksvcs", function.id))
        .map_err(|error| format!("list ksvcs for function {}: {error}", function.id))?;
    let by_name: std::collections::BTreeMap<String, Vec<FaasKsvc>> =
        serde_json::from_slice(&raw).map_err(|error| format!("parse ksvcs: {error}"))?;
    let mut ksvcs = by_name.get(&function.name).cloned().unwrap_or_default();
    if ksvcs.is_empty() {
        // FAAS keys by function name; be tolerant of a different key.
        for values in by_name.values() {
            ksvcs.extend(values.iter().cloned());
        }
    }
    let mut app = TestApp {
        name: function.name.clone(),
        project_path: repo,
        role: "faas-devine-golang".to_string(),
        description: "resolved from the FAAS registry (devine golang only)".to_string(),
        ..TestApp::default()
    };
    for ksvc in &ksvcs {
        if ksvc
            .deleted_at
            .as_deref()
            .is_some_and(|deleted| !deleted.is_empty())
        {
            continue;
        }
        match ksvc.env.as_str() {
            "fat" => {
                if app.fat_group == 0 {
                    app.fat_group = group_id(ksvc);
                    app.version_url = format!("{}/version", ksvc.url.trim_end_matches('/'));
                }
            }
            "prod" => {
                app.prod_groups.push(ProdGroup {
                    id: group_id(ksvc),
                    name: if ksvc.idc.is_empty() {
                        ksvc.region.clone()
                    } else {
                        ksvc.idc.clone()
                    },
                    enabled: true,
                });
                if app.prod_url.is_empty() {
                    app.prod_url = ksvc.url.clone();
                }
            }
            _ => {}
        }
    }
    if app.fat_group == 0 {
        return Err(format!(
            "FAAS app {:?} has no fat ksvc — nothing to resolve the image from",
            function.name
        ));
    }
    if app.prod_groups.is_empty() {
        return Err(format!(
            "FAAS app {:?} has no PROD ksvc/groups provisioned \
             (platform must create the prod app first — CreateProdApp)",
            function.name
        ));
    }
    Ok(app)
}

/// Restriction 1: a devine-golang function, or a refusal.
fn check_devine_golang(function: &FaasFunction) -> Result<(), String> {
    let template_ok = function
        .pipeline_template
        .code
        .starts_with(DEVINE_GOLANG_PIPELINE_PREFIX);
    let language_ok = function.language.is_empty() || function.language == "golang";
    if template_ok && language_ok {
        return Ok(());
    }
    Err(format!(
        "refusing {:?}: only FAAS devine-golang apps are supported \
         (pipeline_template.pipeline_code must start with {DEVINE_GOLANG_PIPELINE_PREFIX:?}) \
         — got pipeline_code={:?} language={:?}",
        function.name, function.pipeline_template.code, function.language
    ))
}

/// Restriction 2: an L4/L5 (non-core) function, or a refusal.
fn check_non_core(function: &FaasFunction) -> Result<(), String> {
    let level = function.importance_level.trim().to_uppercase();
    if level == "L4" || level == "L5" {
        return Ok(());
    }
    Err(format!(
        "refusing {:?}: only NON-CORE FAAS apps are supported \
         (importance_level must be L4 or L5; L1/L2/L3 are core) — got importance_level={:?}",
        function.name, function.importance_level
    ))
}

/// The release-group id, falling back to the ksvc id.
fn group_id(ksvc: &FaasKsvc) -> i64 {
    if ksvc.group_id != 0 {
        ksvc.group_id
    } else {
        ksvc.id
    }
}

#[derive(Debug, Default, Deserialize)]
struct FunctionList {
    #[serde(default)]
    result: Vec<FaasFunction>,
}

/// The FAAS API root, exposed for `deploy-prod` and its tests.
pub const FAAS_BASE_URL: &str = CAPTAIN_BASE_URL;

#[cfg(test)]
mod tests;
