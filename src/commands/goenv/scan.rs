// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The line-level checklist the `ci-go-env` gate applies to a build script.

use std::path::Path;

use regex::Regex;

use crate::output::Severity;

use super::Issue;

/// The environment variables that must never be exported by a build script.
const FORBIDDEN_EXPORTS: [&str; 3] = ["GONOPROXY", "GOPRIVATE", "GONOSUMDB"];

/// The deploy tokens whose literal value must never appear.
const FORBIDDEN_TOKENS: [&str; 2] = ["MCPS_ADMIN_TOKEN", "DEVINE_GIT_TOKEN"];

/// The `${VAR:-<literal>}` shell-default pattern for one variable.
///
/// This is the typical way a deploy token sneaks into a build script: the
/// variable is *technically* overridable, but the default is the secret.
pub(super) fn shell_default_pattern(var: &str) -> Regex {
    crate::static_regex::compile(&format!(r#"\$\{{{var}[:\-]?=?['"]?[^'"}}$]{{4,}}"#))
}

/// Checks one build/CI script against the enforcement checklist.
pub(super) fn scan_env_file(env_path: &Path, env_rel: &str) -> Vec<Issue> {
    let Ok(data) = std::fs::read_to_string(env_path) else {
        return vec![Issue::file(
            env_rel,
            "file-read-error",
            Severity::Error,
            format!("cannot read {env_rel}"),
        )];
    };
    let mut issues = Vec::new();
    let mut goproxy_found = false;
    let mut goproxy_ok = false;

    for (index, raw) in data.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let number = index + 1;

        for var in FORBIDDEN_EXPORTS {
            if has_env_export(line, var) {
                issues.push(Issue::at(
                    env_rel,
                    number,
                    "goproxy-forced",
                    Severity::Error,
                    format!("export {var} forces direct git fetch — use anonymous GOPROXY instead"),
                ));
            }
        }

        for var in FORBIDDEN_TOKENS {
            if is_hardcoded_token(line, var) || shell_default_pattern(var).is_match(line) {
                issues.push(Issue::at(
                    env_rel,
                    number,
                    "git-token-default",
                    Severity::Error,
                    format!("hardcoded {var} default — remove and use anonymous proxy"),
                ));
            }
        }

        if line.contains(r#"url."https://oauth2:"#) {
            issues.push(Issue::at(
                env_rel,
                number,
                "git-oauth2-token",
                Severity::Error,
                "git config with oauth2 literal token — remove git-config plumbing".to_string(),
            ));
        }

        if has_env_export(line, "GOPROXY") {
            goproxy_found = true;
            if line.contains('@') && line.contains("://") {
                issues.push(Issue::at(
                    env_rel,
                    number,
                    "goproxy-credential",
                    Severity::Error,
                    "GOPROXY contains embedded credential (user:pass@) — use anonymous proxy"
                        .to_string(),
                ));
            } else if line.contains("artifactory") {
                goproxy_ok = true;
            }
        }
    }

    if !goproxy_found {
        issues.push(Issue::file(
            env_rel,
            "goproxy-missing",
            Severity::Error,
            "GOPROXY not set in build script".to_string(),
        ));
    } else if !goproxy_ok {
        issues.push(Issue::file(
            env_rel,
            "goproxy-missing",
            Severity::Error,
            "GOPROXY does not reference artifactory — likely not an anonymous proxy chain"
                .to_string(),
        ));
    }
    issues
}

/// Whether `line` is a real environment assignment for `var`.
///
/// A comment line or a value inside an `echo` string is not an assignment.
pub(super) fn has_env_export(line: &str, var: &str) -> bool {
    let needle = format!("{var}=");
    let index = match line.find(&needle) {
        Some(i) => i,
        None => {
            let spaced = format!("{var} =");
            match line.find(&spaced) {
                Some(i) => i,
                None => return false,
            }
        }
    };
    let prefix = line[..index].trim();
    prefix.is_empty() || prefix == "export"
}

/// Whether `line` is `export VAR=<literal>` with a non-trivial literal.
pub(super) fn is_hardcoded_token(line: &str, var: &str) -> bool {
    let needle = format!("export {var}=");
    let Some(index) = line.find(&needle) else {
        return false;
    };
    let mut after = line[index + needle.len()..].trim().to_string();
    if after.is_empty() {
        return false;
    }
    if let Some(rest) = after.strip_prefix('"') {
        after = match rest.find('"') {
            Some(end) => rest[..end].to_string(),
            None => rest.to_string(),
        };
    }
    after = after.trim().to_string();
    if after.is_empty() || after.starts_with('$') {
        return false;
    }
    if let Some(end) = after.find('#') {
        after = after[..end].trim().to_string();
    }
    after.len() >= 3
}
