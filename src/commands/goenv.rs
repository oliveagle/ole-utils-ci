// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `ci-go-env` command: the Go environment regression check.
//!
//! The Go original enforces the checklist from
//! `issue_goproxy_deploy_token_401_20260813.md`: no `GONOPROXY` /
//! `GOPRIVATE` / `GONOSUMDB` exports, no hardcoded deploy tokens, no
//! `oauth2:` plumbing in `git config`, and a `GOPROXY` that actually points
//! at an anonymous Artifactory chain. On top of the build script it scans the
//! whole repository for literal `oauth2:` credentials.

use std::path::Path;

use regex::Regex;

use crate::config::Config;
use crate::output::{Diagnostic, Report, Severity};

use super::goenv_scan::scan_hardcoded_credentials;

/// The build script checked by default.
pub const DEFAULT_ENV_FILE: &str = ".paas/build.sh";

/// The environment variables that must never be exported by a build script.
const FORBIDDEN_EXPORTS: [&str; 3] = ["GONOPROXY", "GOPRIVATE", "GONOSUMDB"];

/// The deploy tokens whose literal value must never appear.
const FORBIDDEN_TOKENS: [&str; 2] = ["MCPS_ADMIN_TOKEN", "DEVINE_GIT_TOKEN"];

/// One finding from the Go-env check.
#[derive(Debug, Clone)]
pub(crate) struct Issue {
    /// Path relative to the project root; empty for whole-file findings.
    path: String,
    /// 1-based line number, `0` when the finding is not line-specific.
    line: usize,
    /// Stable rule identifier.
    rule: &'static str,
    /// How severe the finding is.
    severity: Severity,
    /// One-sentence description.
    msg: String,
}

impl Issue {
    /// Builds a line-specific finding.
    pub(crate) fn at(
        path: &str,
        line: usize,
        rule: &'static str,
        severity: Severity,
        msg: String,
    ) -> Self {
        Issue {
            path: path.to_string(),
            line,
            rule,
            severity,
            msg,
        }
    }

    /// Builds a finding that is not tied to a line.
    pub(crate) fn file(path: &str, rule: &'static str, severity: Severity, msg: String) -> Self {
        Issue::at(path, 0, rule, severity, msg)
    }
}

/// The `${VAR:-<literal>}` shell-default pattern for one variable.
///
/// This is the typical way a deploy token sneaks into a build script: the
/// variable is *technically* overridable, but the default is the secret.
fn shell_default_pattern(var: &str) -> Regex {
    crate::static_regex::compile(&format!(r#"\$\{{{var}[:\-]?=?['"]?[^'"}}$]{{4,}}"#))
}

/// The `ci-go-env` command.
///
/// `env_file` is relative to the project root.
pub fn go_env(cfg: &Config, env_file: &str) -> Result<i32, String> {
    let mut issues = Vec::new();
    let env_path = cfg.root().join(env_file);
    if env_path.exists() {
        issues.extend(scan_env_file(&env_path, env_file));
    } else {
        issues.push(Issue::file(
            "",
            "env-file-missing",
            Severity::Info,
            format!("{env_file} not found — skipping file checks (repo-wide credential scan still runs)"),
        ));
    }
    issues.extend(scan_hardcoded_credentials(cfg.root()));

    if cfg.structured() {
        render_structured(cfg, issues, env_file)
    } else {
        render_text(issues, env_file)
    }
}

/// Checks one build/CI script against the enforcement checklist.
fn scan_env_file(env_path: &Path, env_rel: &str) -> Vec<Issue> {
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
fn has_env_export(line: &str, var: &str) -> bool {
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
fn is_hardcoded_token(line: &str, var: &str) -> bool {
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

/// Renders the legacy human text.
fn render_text(issues: Vec<Issue>, env_file: &str) -> Result<i32, String> {
    println!("🔍 Checking Go environment config ({env_file})...");
    if issues.is_empty() {
        println!("✅ Go environment check passed (no issues)");
        return Ok(0);
    }
    let mut errors = 0usize;
    for issue in &issues {
        if issue.severity == Severity::Info {
            println!("  INFO   {}", issue.msg);
            continue;
        }
        if issue.severity == Severity::Error {
            errors += 1;
        }
        let location = if issue.path.is_empty() {
            String::new()
        } else if issue.line > 0 {
            format!("{}:{}", issue.path, issue.line)
        } else {
            issue.path.clone()
        };
        if location.is_empty() {
            println!("  ERROR  {}: {}", issue.rule, issue.msg);
        } else {
            println!("  ERROR  {location}  {}: {}", issue.rule, issue.msg);
        }
    }
    if errors > 0 {
        return Err(format!("go-env check found {errors} error(s)"));
    }
    println!("✅ Go environment check passed");
    Ok(0)
}

/// Renders the structured payload.
fn render_structured(cfg: &Config, issues: Vec<Issue>, env_file: &str) -> Result<i32, String> {
    let mut report = Report::new("go-env");
    for issue in &issues {
        let mut diagnostic = Diagnostic::info(issue.path.clone(), issue.rule, issue.msg.clone());
        diagnostic.severity = issue.severity;
        diagnostic.line = issue.line;
        report.add(diagnostic);
    }
    report.extra = Some(serde_json::json!({ "env_file": env_file }));
    report.summary = if report.ok {
        "go-env: environment config OK".to_string()
    } else {
        format!("go-env: {} error(s)", report.counts.errors)
    };
    print!("{}", report.render(cfg.output_mode()));
    if report.ok {
        Ok(0)
    } else {
        Err(format!(
            "go-env check found {} error(s)",
            report.counts.errors
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_forbidden_exports() {
        assert!(has_env_export("export GOPRIVATE=example.com", "GOPRIVATE"));
        assert!(has_env_export("GOPRIVATE = x", "GOPRIVATE"));
        assert!(!has_env_export("# export GOPRIVATE=x", "GOPRIVATE"));
        assert!(!has_env_export("echo \"export GOPRIVATE=x\"", "GOPRIVATE"));
        assert!(!has_env_export("export MYGOPRIVATE=x", "GOPRIVATE"));
    }

    #[test]
    fn detects_hardcoded_tokens() {
        assert!(is_hardcoded_token(
            "export DEVINE_GIT_TOKEN=abcdef123",
            "DEVINE_GIT_TOKEN"
        ));
        assert!(!is_hardcoded_token(
            "export DEVINE_GIT_TOKEN=$TOKEN",
            "DEVINE_GIT_TOKEN"
        ));
        assert!(!is_hardcoded_token(
            "export DEVINE_GIT_TOKEN=",
            "DEVINE_GIT_TOKEN"
        ));
        assert!(!is_hardcoded_token(
            "DEVINE_GIT_TOKEN=abcdef",
            "DEVINE_GIT_TOKEN"
        ));
        assert!(!is_hardcoded_token(
            "export DEVINE_GIT_TOKEN=ab",
            "DEVINE_GIT_TOKEN"
        ));
    }

    #[test]
    fn detects_shell_default_tokens() {
        let pattern = shell_default_pattern("MCPS_ADMIN_TOKEN");
        // The Go original quantifies the whole tail after the variable name, so
        // even a two-character default counts as a non-trivial literal.
        assert!(pattern.is_match(r#"export X="${MCPS_ADMIN_TOKEN:-abcdef123}""#));
        assert!(pattern.is_match(r#"export X="${MCPS_ADMIN_TOKEN:-ab}""#));
        assert!(!pattern.is_match(r#"export X="${MCPS_ADMIN_TOKEN}""#));
    }
}
