// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `protect-set` command: make the project follow the documented
//! protection policy — only `main` protected, the two release-tag patterns
//! protected at Developer / Maintainer create level.
//!
//! The Go original parses its own flat flag list (unknown arguments are
//! silently ignored, a value flag at the end of the line is silently
//! dropped) and never fails after the token/project/model checks: every
//! HTTP outcome is reported inline and the command still exits 0.

use std::time::Duration;

use crate::gitlab;
use crate::httpd;

/// The three tag patterns that must not exist before the policy is applied.
const OLD_TAG_PATTERNS: [&str; 3] = ["v[0-9]*.[0-9]*.[0-9]-*", "v[0-9]*.[0-9]*.[0-9]*", "v*"];

/// The branches that must not stay protected — only `main` is protected.
const UNPROTECTED_BRANCHES: [&str; 2] = ["release", "release/*"];

#[derive(Default)]
struct ProtectSet {
    base_url: String,
    project_id: String,
    token: String,
    release_model: String,
}

/// Lifts `--flag value` / `--flag=value` out of the raw argument list the
/// way the Go loop does: anything it does not recognise is kept as-is and
/// then ignored, and `--flag` at the end of the line never consumes a
/// missing value.
fn split_flags(args: &[String]) -> ProtectSet {
    let mut cfg = ProtectSet {
        base_url: "https://git.dev.sh.ctripcorp.com/api/v4".to_string(),
        ..ProtectSet::default()
    };
    const NAMES: [&str; 4] = ["--project-id", "--release-model", "--token", "--base-url"];
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        // The space form: `--flag value`, but only when a value follows.
        if let Some(name) = NAMES.into_iter().find(|name| arg == *name) {
            if index + 1 < args.len() {
                // Go peeks at the next token but still walks over it, so the
                // value is processed again as an argument of its own.
                assign(&mut cfg, name, &args[index + 1]);
            }
            // A value flag at the end of the line consumes nothing.
            index += 1;
            continue;
        }
        // The equals form: `--flag=value`.
        if let Some(name) = NAMES
            .into_iter()
            .find(|name| arg.starts_with(&format!("{name}=")))
        {
            let value = &arg[name.len() + 1..];
            assign(&mut cfg, name, value);
            index += 1;
            continue;
        }
        // Unknown argument: the Go switch falls through and ignores it.
        index += 1;
    }
    cfg
}

/// Stores one flag value; `--base-url` keeps Go's `TrimRight("/")`.
fn assign(cfg: &mut ProtectSet, name: &str, value: &str) {
    match name {
        "--project-id" => cfg.project_id = value.to_string(),
        "--release-model" => cfg.release_model = value.to_string(),
        "--token" => cfg.token = value.to_string(),
        "--base-url" => cfg.base_url = value.trim_end_matches('/').to_string(),
        _ => {}
    }
}

/// One API exchange; the status is all the caller needs, so transport errors
/// collapse to status 0 exactly like the Go `code, _ := call_api(...)` calls.
fn call_api(base_url: &str, token: &str, method: &str, path: &str, body: Option<&str>) -> u16 {
    let url = format!("{base_url}{path}");
    let payload = body.map(|text| {
        let mut with_newline = text.to_string();
        with_newline.push('\n');
        with_newline
    });
    let bytes = payload.as_ref().map(String::as_bytes);
    match httpd::request(
        method,
        &url,
        &[
            ("PRIVATE-TOKEN", token),
            ("Content-Type", "application/json"),
        ],
        bytes,
        Duration::from_secs(30),
    ) {
        Ok(response) => response.status,
        Err(_) => 0,
    }
}

/// The `protect-set` command.
pub fn protect_set(args: &[String]) -> Result<i32, String> {
    let mut cfg = split_flags(args);

    if cfg.token.is_empty() {
        cfg.token = gitlab::dotvault_get("DEVINE_GIT_GROUP_ACCESS_KEY")
            .map_err(|_| "no token: set --token or run dotvault install".to_string())?;
    }
    if cfg.project_id.is_empty() {
        return Err("--project-id is required".to_string());
    }
    crate::commands::validate_release_model(&cfg.release_model)?;

    let base = cfg.base_url.as_str();
    let token = cfg.token.as_str();
    let project = cfg.project_id.as_str();

    // Drop whatever protection the project carries today; every failure here
    // is ignored because the goal is the end state, not the transitions.
    for pattern in OLD_TAG_PATTERNS {
        let encoded = gitlab::path_escape(pattern);
        call_api(
            base,
            token,
            "DELETE",
            &format!("/projects/{project}/protected_tags/{encoded}"),
            None,
        );
    }
    call_api(
        base,
        token,
        "DELETE",
        &format!("/projects/{project}/protected_branches/main"),
        None,
    );

    let code = call_api(
        base,
        token,
        "POST",
        &format!("/projects/{project}/protected_branches"),
        Some(
            r#"{"name":"main","push_access_level":0,"merge_access_level":30,"allow_force_push":false}"#,
        ),
    );
    if code == 201 {
        println!("  main: push=0 merge=30 ✅");
    } else {
        println!("  main: push=0 merge=30 ⚠️HTTP {code}");
    }

    // devine_groups "只保护 main" rule: release / release/v* stay open.
    for branch in UNPROTECTED_BRANCHES {
        let encoded = gitlab::path_escape(branch);
        let code = call_api(
            base,
            token,
            "DELETE",
            &format!("/projects/{project}/protected_branches/{encoded}"),
            None,
        );
        if code == 204 || code == 404 {
            println!("  {branch}: unprotected (only main is protected) ✅");
        } else {
            println!("  {branch}: unprotect ⚠️HTTP {code}");
        }
    }

    let code = call_api(
        base,
        token,
        "POST",
        &format!("/projects/{project}/protected_tags"),
        Some(r#"{"name":"v[0-9]*.[0-9]*.[0-9]-*","create_access_level":30}"#),
    );
    if code == 201 {
        println!("  tag v[...]-*: create=30 (Developer) ✅");
    } else {
        println!("  tag v[...]-*: create=30 ⚠️HTTP {code}");
    }

    let code = call_api(
        base,
        token,
        "POST",
        &format!("/projects/{project}/protected_tags"),
        Some(r#"{"name":"v[0-9]*.[0-9]*.[0-9]*","create_access_level":40}"#),
    );
    if code == 201 {
        println!("  tag v[...]*: create=40 (Maintainer) ✅");
    } else {
        println!("  tag v[...]*: create=40 ⚠️HTTP {code}");
    }

    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn accepts_both_flag_forms() {
        let cfg = split_flags(&args(&[
            "--project-id=7",
            "--release-model",
            "strict",
            "--token",
            "sekrit",
            "--base-url=https://git.example/api/v4/",
        ]));
        assert_eq!(cfg.project_id, "7");
        assert_eq!(cfg.release_model, "strict");
        assert_eq!(cfg.token, "sekrit");
        // The trailing slash is trimmed the way Go's TrimRight does.
        assert_eq!(cfg.base_url, "https://git.example/api/v4");
    }

    #[test]
    fn ignores_unknown_arguments_and_dangling_values() {
        let cfg = split_flags(&args(&[
            "positional",
            "--wat",
            "value",
            "--project-id",
            "--release-model=simple",
        ]));
        // Go peeks at the token after `--project-id` without skipping it, so
        // the value is stored twice: once as the id, once as its own flag.
        assert_eq!(cfg.project_id, "--release-model=simple");
        assert_eq!(cfg.release_model, "simple");
        assert_eq!(cfg.base_url, "https://git.dev.sh.ctripcorp.com/api/v4");
    }

    #[test]
    fn model_validation_matches_go() {
        assert!(crate::commands::validate_release_model("simple").is_ok());
        assert!(crate::commands::validate_release_model("strict").is_ok());
        let err = crate::commands::validate_release_model("").unwrap_err();
        assert_eq!(err, "unknown release model \"\" — use 'simple' or 'strict'");
        let err = crate::commands::validate_release_model("bogus").unwrap_err();
        assert_eq!(
            err,
            "unknown release model \"bogus\" — use 'simple' or 'strict'"
        );
    }
}
