// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `artifact-size` gate: release-artifact size budgets.
//!
//! The gate is inert until a project configures artifact globs, so adding it
//! to `all` cannot surprise a repository that has no release artifacts yet.

use crate::config::{limits, Config};
use crate::fsutil;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report, Severity};
use crate::quality_config::{QualityArtifactSizeRule, QualityConfig};

/// A rule with its defaults filled in.
#[derive(Debug, Clone)]
pub struct ResolvedRule {
    /// The glob the rule matches.
    pub glob: String,
    /// The warning budget, in MiB.
    pub warn_mib: f64,
    /// The hard budget, in MiB.
    pub max_mib: f64,
    /// Whether the artifact must exist.
    pub required: bool,
}

/// One artifact that matched a rule.
#[derive(Debug, Clone)]
pub struct ArtifactMatch {
    /// Path relative to the project root.
    pub path: String,
    /// The artifact's size in bytes.
    pub size: u64,
    /// The rule it matched.
    pub rule: ResolvedRule,
}

/// Resolves the configured rules, filling in the built-in budgets.
pub fn resolve_rules(qcfg: &QualityConfig) -> Result<Vec<ResolvedRule>, String> {
    let Some(cfg) = &qcfg.artifact_size else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (index, rule) in cfg.paths.iter().enumerate() {
        out.push(resolve_rule(rule, index)?);
    }
    Ok(out)
}

fn resolve_rule(rule: &QualityArtifactSizeRule, index: usize) -> Result<ResolvedRule, String> {
    if rule.glob.trim().is_empty() {
        return Err(format!(
            "artifact-size rule {} has an empty glob",
            index + 1
        ));
    }
    if rule.warn_mb < 0.0 || rule.max_mb < 0.0 {
        return Err(format!(
            "artifact-size rule {:?} has a negative limit",
            rule.glob
        ));
    }
    let warn_defaulted = rule.warn_mb == 0.0;
    let mut warn_mib = rule.warn_mb;
    let mut max_mib = rule.max_mb;
    if warn_mib == 0.0 {
        warn_mib = limits::HARD_ARTIFACT_WARN_MIB as f64;
    }
    if max_mib == 0.0 {
        max_mib = limits::HARD_ARTIFACT_MAX_MIB as f64;
    }
    if warn_defaulted && warn_mib > max_mib {
        warn_mib = max_mib;
    }
    if warn_mib > limits::HARD_ARTIFACT_WARN_MIB as f64
        || max_mib > limits::HARD_ARTIFACT_MAX_MIB as f64
    {
        return Err(format!(
            "artifact-size rule {:?} can only tighten built-in warn={} max={} MiB",
            rule.glob,
            limits::HARD_ARTIFACT_WARN_MIB,
            limits::HARD_ARTIFACT_MAX_MIB
        ));
    }
    if warn_mib > max_mib {
        return Err(format!(
            "artifact-size rule {:?} has warn-mb {warn_mib:.2} above max-mb {max_mib:.2}",
            rule.glob
        ));
    }
    Ok(ResolvedRule {
        glob: rule.glob.clone(),
        warn_mib,
        max_mib,
        required: rule.required,
    })
}

/// Scans the repository for artifacts that match the rules.
pub fn scan(root: &std::path::Path, rules: &[ResolvedRule]) -> (Report, Vec<ArtifactMatch>) {
    let mut report = Report::new("artifact-size");
    let mut matches = Vec::new();
    let mut found = vec![0usize; rules.len()];
    for entry in fsutil::walk_files(root) {
        let Some(size) = std::fs::metadata(&entry.path).map(|m| m.len()).ok() else {
            continue;
        };
        for (index, rule) in rules.iter().enumerate() {
            if !crate::quality_config::match_glob_pattern(&entry.rel, &rule.glob) {
                continue;
            }
            found[index] += 1;
            matches.push(ArtifactMatch {
                path: entry.rel.clone(),
                size,
                rule: rule.clone(),
            });
            if let Some(diagnostic) = size_diagnostic(&entry.rel, size, rule) {
                report.add(diagnostic);
            }
        }
    }
    add_missing_diagnostics(&mut report, rules, &found);
    (report, matches)
}

fn size_diagnostic(path: &str, size: u64, rule: &ResolvedRule) -> Option<Diagnostic> {
    let mib = size as f64 / (1024.0 * 1024.0);
    if rule.max_mib > 0.0 && mib > rule.max_mib {
        return Some(Diagnostic::error(
            path,
            "artifact-size.max",
            format!("artifact is {mib:.2} MiB (max {:.2} MiB)", rule.max_mib),
        ));
    }
    if rule.warn_mib > 0.0 && mib > rule.warn_mib {
        return Some(Diagnostic::warning(
            path,
            "artifact-size.warn",
            format!("artifact is {mib:.2} MiB (warn {:.2} MiB)", rule.warn_mib),
        ));
    }
    None
}

fn add_missing_diagnostics(report: &mut Report, rules: &[ResolvedRule], found: &[usize]) {
    for (index, rule) in rules.iter().enumerate() {
        if rule.required && found[index] == 0 {
            report.add(Diagnostic::error(
                rule.glob.clone(),
                "artifact-size.missing",
                "required artifact glob matched no files".to_string(),
            ));
        }
    }
}

/// Runs the gate and returns its result.
pub fn run(cfg: &Config) -> Result<GateResult, String> {
    let qcfg = QualityConfig::load(cfg.root())?.unwrap_or_default();
    validate_config(&qcfg)?;
    let rules = resolve_rules(&qcfg)?;
    let (mut report, matches) = scan(cfg.root(), &rules);
    report.command = "artifact-size".to_string();
    let errors = report.counts.errors;
    // Sorted keys, the way Go's `map[string]any` encodes them.
    report.extra = Some(serde_json::json!({
        "matches": matches.len(),
        "rules": rules.len(),
    }));
    report.summary = format!(
        "artifact-size: {} rule(s), {} match(es), {errors} error(s), {} warning(s)",
        rules.len(),
        matches.len(),
        report.counts.warnings
    );
    if report.ok {
        Ok(GateResult::finish(report))
    } else {
        let message = format!("{errors} artifact-size violation(s)");
        Ok(
            GateResult::failed("artifact-size", report.summary.clone(), report)
                .failing(message.clone(), message),
        )
    }
}

/// Rejects a config that tries to exclude paths from a hard budget.
pub fn validate_config(qcfg: &QualityConfig) -> Result<(), String> {
    if let Some(section) = &qcfg.ignore.artifact_size {
        if !section.paths.is_empty() {
            return Err(
                "artifact-size: path ignores are not allowed for hard quality budgets; lower the budget instead"
                    .to_string(),
            );
        }
    }
    Ok(())
}

/// The human-readable body printed when `--output` is not set.
pub fn text_body(result: &GateResult) -> String {
    let extra = result.report.extra.as_ref();
    let number = |name: &str| {
        extra
            .and_then(|payload| payload.get(name))
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    };
    let mut body = format!(
        "📦 Checking release artifact budgets ({} rule(s), {} match(es))...\n",
        number("rules"),
        number("matches")
    );
    for d in &result.report.diagnostics {
        let mark = match d.severity {
            Severity::Error => "✗",
            Severity::Warning => "⚠",
            Severity::Info => "·",
        };
        body.push_str(&format!("  {mark} {}: {}\n", d.path, d.message));
    }
    if result.passed() {
        body.push_str(&format!(
            "✅ Artifact size check passed with {} warning(s)\n",
            result.report.counts.warnings
        ));
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_in_the_built_in_budgets() {
        let rule = QualityArtifactSizeRule {
            glob: "dist/*".to_string(),
            ..Default::default()
        };
        let resolved = resolve_rule(&rule, 0).unwrap();
        assert_eq!(resolved.warn_mib, 50.0);
        assert_eq!(resolved.max_mib, 80.0);
    }

    #[test]
    fn rejects_budgets_above_the_built_in_ones() {
        let rule = QualityArtifactSizeRule {
            glob: "dist/*".to_string(),
            max_mb: 500.0,
            ..Default::default()
        };
        assert!(resolve_rule(&rule, 0).is_err());
    }

    #[test]
    fn rejects_an_empty_glob() {
        let rule = QualityArtifactSizeRule::default();
        assert!(resolve_rule(&rule, 0).is_err());
    }
}
