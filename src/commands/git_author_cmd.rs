// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `git-author` command line: sub-command parsing and report rendering.
//!
//! The policy itself lives in [`super::git_author`]; this module is the thin
//! layer that lifts `--config` and the positional sub-command out of the flag
//! stream (Go's `flag` package stops at the first non-flag argument) and then
//! renders the verdict in whichever format the caller asked for.

use crate::config::Config;
use crate::output::{Diagnostic, Report, Severity};

use super::git_author::{check, gather, load_config, Verdict};

/// Splits the `git-author` argument list into the sub-command, the
/// `--config` path and the flags left for the global parser.
///
/// Go's `flag` package stops at the first non-flag argument, so the
/// positional has to be lifted out or a later `--output` would be dropped.
pub fn split_args(args: &[String]) -> (String, String, Vec<String>) {
    let mut config_path = String::new();
    let mut sub = String::new();
    let mut rest = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--config" && index + 1 < args.len() {
            config_path = args[index + 1].clone();
            index += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--config=") {
            config_path = value.to_string();
            index += 1;
            continue;
        }
        if sub.is_empty() && !arg.starts_with('-') {
            sub = arg.clone();
            index += 1;
            continue;
        }
        rest.push(arg.clone());
        index += 1;
    }
    if sub.is_empty() {
        sub = "show".to_string();
    }
    (sub, config_path, rest)
}

/// The `git-author` command.
pub fn git_author(cfg: &Config, sub: &str, config_path: &str) -> Result<i32, String> {
    let config = load_config(config_path)?;
    let id = gather(cfg.root());
    let verdict = check(&config, &id);

    let mut report = Report::new("git-author");
    let mut applied: Option<(String, String)> = None;

    match sub {
        "show" => {
            report.ok = true;
            match (&verdict.rule, verdict.ok) {
                (None, _) => {
                    report.summary = format!(
                        "git-author: remote {} — no rule matched (identity not enforced)",
                        id.remote
                    );
                }
                (Some(_), true) => {
                    report.summary = format!(
                        "git-author: {} satisfies rule {:?}",
                        id.email,
                        description(&verdict)
                    );
                }
                _ => {
                    report.summary = format!("git-author: {}", verdict.reason);
                    report.add(Diagnostic {
                        severity: Severity::Warning,
                        path: String::new(),
                        line: 0,
                        col: 0,
                        rule: "git-author".to_string(),
                        message: verdict.reason.clone(),
                        detail: hint(&verdict),
                    });
                }
            }
        }
        "check" => {
            if verdict.ok {
                report.ok = true;
                if verdict.rule.is_none() {
                    report.summary = "git-author: no rule for this remote (skipped)".to_string();
                } else {
                    report.summary = format!(
                        "git-author: {} satisfies rule {:?}",
                        id.email,
                        description(&verdict)
                    );
                }
            } else {
                report.ok = false;
                report.summary = "git-author: identity does not match required rule".to_string();
                report.add(Diagnostic {
                    severity: Severity::Error,
                    path: String::new(),
                    line: 0,
                    col: 0,
                    rule: "git-author".to_string(),
                    message: verdict.reason.clone(),
                    detail: hint(&verdict),
                });
            }
        }
        "fix" => {
            if verdict.rule.is_none() {
                report.ok = true;
                report.summary = "git-author: no rule matched — nothing to fix".to_string();
            } else if verdict.ok {
                report.ok = true;
                report.summary = format!(
                    "git-author: {} already satisfies rule {:?} — nothing to fix",
                    id.email,
                    description(&verdict)
                );
            } else {
                let root = cfg.root();
                crate::git::run(
                    root,
                    &["config", "--local", "user.email", &verdict.expected_email],
                )
                .map_err(|e| format!("git-author: set user.email failed: {e}"))?;
                if !verdict.expected_name.is_empty() {
                    crate::git::run(
                        root,
                        &["config", "--local", "user.name", &verdict.expected_name],
                    )
                    .map_err(|e| format!("git-author: set user.name failed: {e}"))?;
                }
                report.ok = true;
                report.summary = format!(
                    "git-author: repo-local identity updated to {} <{}>",
                    verdict.expected_name, verdict.expected_email
                );
                applied = Some((
                    verdict.expected_email.clone(),
                    verdict.expected_name.clone(),
                ));
            }
        }
        other => {
            return Err(format!(
                "git-author: unknown subcommand {other:?} — use show, check, or fix"
            ));
        }
    }

    // The Go original marshals a `map[string]any`, whose keys are sorted;
    // build the payload in the same order so `--output json` matches.
    let mut fields = serde_json::Map::new();
    if let Some((email, name)) = applied {
        fields.insert("applied_email".to_string(), serde_json::json!(email));
        fields.insert("applied_name".to_string(), serde_json::json!(name));
    }
    for (key, value) in [
        ("current_email", serde_json::json!(id.email)),
        ("current_name", serde_json::json!(id.name)),
        ("expected_email", serde_json::json!(verdict.expected_email)),
        ("expected_name", serde_json::json!(verdict.expected_name)),
        ("local_scope", serde_json::json!(id.local)),
        (
            "matched_rule",
            serde_json::to_value(&verdict.rule).unwrap_or(serde_json::Value::Null),
        ),
        ("mode", serde_json::json!(sub)),
        ("remote", serde_json::json!(id.remote)),
        (
            "rules_evaluated",
            serde_json::to_value(&config.rules).unwrap_or_else(|_| serde_json::json!([])),
        ),
    ] {
        fields.insert(key.to_string(), value);
    }
    report.extra = Some(serde_json::Value::Object(fields));
    if cfg.structured() {
        print!("{}", report.render(cfg.output_mode()));
    } else {
        if !report.summary.is_empty() {
            println!("{}", report.summary);
        }
        if !report.ok {
            for diagnostic in &report.diagnostics {
                if !diagnostic.detail.is_empty() {
                    println!("  hint: {}", diagnostic.detail);
                }
            }
        }
    }
    if report.ok {
        Ok(0)
    } else {
        Err("git-author: identity violates policy".to_string())
    }
}

/// The `hint:` line printed under a failing summary.
fn hint(verdict: &Verdict) -> String {
    format!(
        "expected email={:?} name={:?}; run `devine-utils-ci git-author fix` to auto-correct",
        verdict.expected_email, verdict.expected_name
    )
}

/// The rule description quoted the way the Go original printed it with `%q`.
fn description(verdict: &Verdict) -> String {
    verdict
        .rule
        .as_ref()
        .map(|rule| rule.description.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_subcommand_is_lifted_out_of_the_flag_stream() {
        let args: Vec<String> = ["check", "--output", "json"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (sub, config, rest) = split_args(&args);
        assert_eq!(sub, "check");
        assert!(config.is_empty());
        assert_eq!(rest, vec!["--output".to_string(), "json".to_string()]);
    }

    #[test]
    fn the_config_flag_is_hoisted() {
        let args: Vec<String> = ["--config", "/tmp/a.yaml", "fix"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (sub, config, rest) = split_args(&args);
        assert_eq!(sub, "fix");
        assert_eq!(config, "/tmp/a.yaml");
        assert!(rest.is_empty());
    }
}
