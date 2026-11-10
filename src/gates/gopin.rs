// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `go-pin` gate: Go toolchain drift.
//!
//! `.go-version` is the single source of truth; every version-bearing
//! surface — the CI image, the Dockerfile, the `go` directive in `go.mod`,
//! the `require-go` script — must agree with it.

use std::path::Path;

use regex::Regex;

use crate::config::Config;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};

/// The default name of the version file.
pub const DEFAULT_VERSION_FILE: &str = ".go-version";
/// The default name of the CI file.
pub const DEFAULT_CI_FILE: &str = ".gitlab-ci.yml";
/// The default name of the Dockerfile.
pub const DEFAULT_DOCKERFILE: &str = "Dockerfile";
/// The default name of the `require-go` script.
pub const DEFAULT_REQUIRE_GO_SCRIPT: &str = "scripts/ci/require-go.sh";

/// One drift finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinFinding {
    /// The surface that disagrees.
    pub path: String,
    /// The rule that was broken.
    pub rule: String,
    /// A one-line description.
    pub message: String,
}

fn semver_re() -> Regex {
    Regex::new(r"^\d+\.\d+\.\d+$").expect("static semver regex must compile")
}

fn go_image_re() -> Regex {
    Regex::new(r#"(?m)^\s*GO_IMAGE:\s*"[^"]*golang:(\d+\.\d+\.\d+)""#)
        .expect("static GO_IMAGE regex must compile")
}

fn go_directive_re() -> Regex {
    Regex::new(r"(?m)^go\s+(\d+\.\d+)").expect("static go directive regex must compile")
}

fn docker_from_re() -> Regex {
    Regex::new(r"(?m)^\s*FROM\s+\S*golang:(\d+\.\d+\.\d+)").expect("static FROM regex must compile")
}

fn required_go_re() -> Regex {
    Regex::new(r#"REQUIRED_GO_VERSION="\d+\.\d+\.\d+""#)
        .expect("static REQUIRED_GO_VERSION regex must compile")
}

/// Checks the toolchain pin across every surface.
pub fn check(
    root: &Path,
    version_file: &str,
    ci_file: &str,
    dockerfile: &str,
    require_go_script: &str,
) -> (Vec<PinFinding>, Option<String>) {
    let mut findings = Vec::new();
    let version_path = root.join(version_file);
    let Ok(raw) = std::fs::read_to_string(&version_path) else {
        findings.push(PinFinding {
            path: version_file.to_string(),
            rule: "go-pin/version-missing".to_string(),
            message: "Go version source file is missing".to_string(),
        });
        return (findings, None);
    };
    let want = raw.trim().to_string();
    if !semver_re().is_match(&want) {
        findings.push(PinFinding {
            path: version_file.to_string(),
            rule: "go-pin/version-format".to_string(),
            message: format!("must be full semver x.y.z, got {want:?}"),
        });
        return (findings, None);
    }
    let want_mm = match want.rfind('.') {
        Some(idx) => want[..idx].to_string(),
        None => want.clone(),
    };
    findings.extend(check_ci(root, ci_file, &want));
    findings.extend(check_dockerfile(root, dockerfile, &want));
    findings.extend(check_gomod(root, &want_mm));
    findings.extend(check_require_go(root, require_go_script, &want));
    (findings, Some(want))
}

fn check_ci(root: &Path, ci_file: &str, want: &str) -> Vec<PinFinding> {
    let path = root.join(ci_file);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return vec![PinFinding {
            path: ci_file.to_string(),
            rule: "go-pin/ci-missing".to_string(),
            message: "CI file is missing".to_string(),
        }];
    };
    let mut findings = Vec::new();
    match go_image_re().captures(&content) {
        None => findings.push(PinFinding {
            path: ci_file.to_string(),
            rule: "go-pin/ci-image-missing".to_string(),
            message: "GO_IMAGE with a golang:x.y.z tag was not found".to_string(),
        }),
        Some(caps) => {
            let found = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            if found != want {
                findings.push(PinFinding {
                    path: ci_file.to_string(),
                    rule: "go-pin/ci-image".to_string(),
                    message: format!("GO_IMAGE uses golang:{found}, expected golang:{want}"),
                });
            }
        }
    }
    findings
}

fn check_dockerfile(root: &Path, dockerfile: &str, want: &str) -> Vec<PinFinding> {
    let path = root.join(dockerfile);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for caps in docker_from_re().captures_iter(&content) {
        let found = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
        if found != want {
            findings.push(PinFinding {
                path: dockerfile.to_string(),
                rule: "go-pin/docker-image".to_string(),
                message: format!("Dockerfile uses golang:{found}, expected golang:{want}"),
            });
        }
    }
    findings
}

fn check_gomod(root: &Path, want_mm: &str) -> Vec<PinFinding> {
    let path = root.join("go.mod");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for caps in go_directive_re().captures_iter(&content) {
        let found = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
        if found != want_mm {
            findings.push(PinFinding {
                path: "go.mod".to_string(),
                rule: "go-pin/go-directive".to_string(),
                message: format!("go.mod declares go {found}, expected {want_mm}"),
            });
        }
    }
    findings
}

fn check_require_go(root: &Path, script: &str, want: &str) -> Vec<PinFinding> {
    let path = root.join(script);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for caps in required_go_re().captures_iter(&content) {
        let found = caps
            .get(0)
            .map(|m| m.as_str())
            .unwrap_or_default()
            .trim_matches('"')
            .rsplit('=')
            .next()
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        if found != want {
            findings.push(PinFinding {
                path: script.to_string(),
                rule: "go-pin/require-go".to_string(),
                message: format!("{script} pins {found}, expected {want}"),
            });
        }
    }
    findings
}

/// Runs the gate and returns its result.
pub fn run(cfg: &Config) -> Result<GateResult, String> {
    let (findings, version) = check(
        cfg.root(),
        DEFAULT_VERSION_FILE,
        DEFAULT_CI_FILE,
        DEFAULT_DOCKERFILE,
        DEFAULT_REQUIRE_GO_SCRIPT,
    );
    let mut report = Report::new("go-pin");
    report.extra = Some(serde_json::json!({
        "version": version,
        "version_file": DEFAULT_VERSION_FILE,
    }));
    for f in &findings {
        report.add(Diagnostic::error(
            f.path.clone(),
            f.rule.clone(),
            f.message.clone(),
        ));
    }
    if report.ok {
        report.summary = "go-pin: toolchain pin is consistent".to_string();
        Ok(GateResult::finish(report))
    } else {
        report.summary = format!("go-pin: {} finding(s)", findings.len());
        Ok(GateResult::failed("go-pin", report.summary.clone(), report))
    }
}

/// The human-readable body printed when `--output` is not set.
pub fn text_body(result: &GateResult) -> String {
    let mut body = String::from("📌 Checking Go toolchain pin...\n");
    for d in &result.report.diagnostics {
        body.push_str(&format!("  ✗ {}: {}\n", d.path, d.message));
    }
    if result.passed() {
        body.push_str("✅ Go toolchain pin is consistent\n");
    } else {
        body.push_str(&format!(
            "✗ {} toolchain drift finding(s)\n",
            result.report.diagnostics.len()
        ));
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_semver_versions() {
        assert!(semver_re().is_match("1.25.11"));
        assert!(!semver_re().is_match("1.25"));
        assert!(!semver_re().is_match("v1.25.11"));
    }

    #[test]
    fn finds_the_go_image() {
        let re = go_image_re();
        let caps = re
            .captures(r#"  GO_IMAGE: "golang:1.25.11""#)
            .expect("image should match");
        assert_eq!(caps.get(1).map(|m| m.as_str()), Some("1.25.11"));
    }
}
