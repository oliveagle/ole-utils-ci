// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `workflow status`: a one-shot, read-only snapshot of the release and test
//! topology.
//!
//! It answers "where are we right now?": the binary repo's VERSION, branch,
//! HEAD and latest tags, which of those versions PROD artifactory actually
//! serves, and what each test app reports on `/version`. Every probe is best
//! effort — running outside the binary repo still reports the apps.

use serde::Serialize;

use crate::runner::Runner;

use super::flags::{parse_with, scan_int, Take, WorkflowFlags};
use super::http::http_head;
use super::http::probe_app_version;
use super::model::WorkflowConfig;
use super::registry::Context;
use super::util::{
    artifactory_url, current_branch, ensure_binary_repo_dir, go_slice, non_empty_or,
    read_version_from_file, run_git, write_output_json,
};

/// The binary repo snapshot.
#[derive(Debug, Clone, Serialize)]
struct RepoInfo {
    version: String,
    branch: String,
    head: String,
    tags: Vec<String>,
}

/// One app's `/version` answer.
#[derive(Debug, Clone, Serialize)]
struct AppInfo {
    name: String,
    role: String,
    debug: bool,
    app_version: String,
    #[serde(skip_serializing_if = "is_zero")]
    http_status: u16,
    version_url: String,
}

/// The whole snapshot.
#[derive(Debug, Clone, Serialize)]
struct StatusOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    binary_repo: Option<RepoInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    artifactory_prod_versions: Vec<String>,
    test_apps: Vec<AppInfo>,
}

/// The flags of `workflow status`.
#[derive(Debug, Clone)]
pub struct StatusFlags {
    common: WorkflowFlags,
    tags: i64,
}

impl Default for StatusFlags {
    fn default() -> StatusFlags {
        StatusFlags {
            common: WorkflowFlags::default(),
            tags: 3,
        }
    }
}

fn parse_status(args: &[String]) -> (StatusFlags, Vec<String>) {
    let mut flags = StatusFlags::default();
    let rest = parse_with(args, |arg, next| {
        if let Some(take) = flags.common.take_common(arg, next) {
            return Some(take);
        }
        match (arg, next) {
            ("--tags", Some(value)) => {
                flags.tags = scan_int(value);
                Some(Take::Value)
            }
            _ => None,
        }
    });
    (flags, rest)
}

/// `workflow status`.
pub fn workflow_status(
    args: &[String],
    cfg: &WorkflowConfig,
    ctx: &Context,
) -> Result<i32, String> {
    let (flags, positional) = parse_status(args);
    if !positional.is_empty() {
        return Err(format!(
            "status: unexpected positional args {}",
            go_slice(&positional)
        ));
    }

    let mut out = StatusOut {
        binary_repo: None,
        artifactory_prod_versions: Vec::new(),
        test_apps: Vec::new(),
    };
    if let Ok(repo_dir) = ensure_binary_repo_dir(cfg, &ctx.cwd) {
        out.binary_repo = Some(repo_snapshot(&repo_dir, cfg, &flags));
    }
    for version in candidate_versions(out.binary_repo.as_ref()) {
        let url = artifactory_url(cfg, &cfg.artifactory.prod_path, &version);
        if http_head(&url).present {
            out.artifactory_prod_versions.push(version);
        }
    }
    for app in &cfg.test_apps {
        let probe = probe_app_version(&app.version_url);
        out.test_apps.push(AppInfo {
            name: app.name.clone(),
            role: app.role.clone(),
            debug: app.debug,
            app_version: probe.got,
            http_status: probe.status,
            version_url: app.version_url.clone(),
        });
    }

    if flags.common.output == "json" {
        write_output_json(&out)?;
        return Ok(0);
    }
    render_text(cfg, &out);
    Ok(0)
}

/// The local binary repo snapshot, each field best effort.
fn repo_snapshot(dir: &std::path::Path, cfg: &WorkflowConfig, flags: &StatusFlags) -> RepoInfo {
    let runner = Runner::new(dir, flags.common.verbose);
    let mut info = RepoInfo {
        version: String::new(),
        branch: String::new(),
        head: String::new(),
        tags: Vec::new(),
    };
    if let Ok(version) = read_version_from_file(dir, &cfg.binary_repo.version_file) {
        info.version = version;
    }
    if let Ok(branch) = current_branch(&runner) {
        info.branch = branch;
    }
    if let Ok(head) = run_git(&runner, &["rev-parse", "--short", "HEAD"]) {
        info.head = head.trim().to_string();
    }
    // Latest N tags, newest first, limited to vX.Y.Z and vX.Y.Z-rc.N.
    if let Ok(tags) = run_git(
        &runner,
        &["tag", "--sort=-version:refname", "-l", "v[0-9]*"],
    ) {
        for tag in tags.lines() {
            let tag = tag.trim();
            if tag.is_empty() {
                continue;
            }
            info.tags.push(tag.to_string());
            if info.tags.len() as i64 >= flags.tags {
                break;
            }
        }
    }
    info
}

/// The PROD versions worth probing: the current VERSION plus the newest tags,
/// de-duplicated, capped at three and sorted.
fn candidate_versions(repo: Option<&RepoInfo>) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    if let Some(repo) = repo {
        if !repo.version.is_empty() {
            all.push(repo.version.clone());
        }
        for tag in &repo.tags {
            let version = tag.trim_start_matches('v');
            if !version.is_empty() && version != repo.version {
                all.push(version.to_string());
            }
        }
    }
    let mut seen: Vec<String> = Vec::new();
    let mut unique: Vec<String> = Vec::new();
    for version in all {
        if seen.contains(&version) {
            continue;
        }
        seen.push(version.clone());
        unique.push(version);
    }
    unique.truncate(3);
    unique.sort();
    unique
}

/// The human-facing rendering.
fn render_text(cfg: &WorkflowConfig, out: &StatusOut) {
    match &out.binary_repo {
        Some(repo) => {
            println!("Binary repo: {}", cfg.binary_repo.name);
            println!("  VERSION   {}", non_empty_or(&repo.version, "(unknown)"));
            println!("  branch    {}", non_empty_or(&repo.branch, "(unknown)"));
            println!("  HEAD      {}", non_empty_or(&repo.head, "(unknown)"));
            if !repo.tags.is_empty() {
                println!("  tags      {}", repo.tags.join(", "));
            }
        }
        None => println!(
            "Binary repo: {} (not a checkout of this repo in cwd — status limited)",
            cfg.binary_repo.name
        ),
    }
    if !out.artifactory_prod_versions.is_empty() {
        println!(
            "  PROD artifactory: {}",
            out.artifactory_prod_versions.join(", ")
        );
    }
    println!();
    println!(
        "{:<40} {:<14} {:<6}  {:<22}  http",
        "app", "role", "debug", "app_version"
    );
    for app in &out.test_apps {
        let debug = if app.debug { "on" } else { "off" };
        let status = if app.http_status > 0 {
            app.http_status.to_string()
        } else {
            "n/a".to_string()
        };
        let version = if app.app_version.is_empty() {
            "(unreachable)"
        } else {
            app.app_version.as_str()
        };
        println!(
            "{:<40} {:<14} {:<6}  {:<22}  {}",
            app.name, app.role, debug, version, status
        );
    }
}

/// Serde helper: `json:"http_status,omitempty"`.
fn is_zero(value: &u16) -> bool {
    *value == 0
}

#[cfg(test)]
mod tests;
