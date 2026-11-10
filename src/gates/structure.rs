// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `structure` gate: keeps the repository root tidy.
//!
//! Three rules are checked: no stray executables in the root, the required
//! top-level files exist, and build-artifact directories are ignored by git.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::config::Config;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};

/// One structural problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureIssue {
    /// The offending path, relative to the project root.
    pub path: String,
    /// The rule that was broken.
    pub rule: String,
    /// A one-line description.
    pub message: String,
}

/// File names that may be executable in the repository root.
const ALLOWED_EXECUTABLE_SUFFIXES: [&str; 11] = [
    "Makefile",
    "Justfile",
    "justfile",
    "Dockerfile",
    ".sh",
    ".md",
    ".txt",
    ".yml",
    ".yaml",
    ".json",
    ".toml",
];

/// Files every project must have at its root, whatever the language.
const REQUIRED_FILES: [&str; 1] = ["README.md"];

/// The manifests a project may carry. Exactly one of them is expected, so
/// the presence of either satisfies the rule.
const MANIFEST_FILES: [&str; 2] = ["go.mod", "Cargo.toml"];

/// Directories that are build artifacts and must be git-ignored.
const BUILD_ARTIFACT_DIRS: [&str; 5] = ["_build", "bin", "dist", ".go-mod", "target"];

/// Whether a root file may be executable.
pub fn is_allowed_executable(name: &str) -> bool {
    ALLOWED_EXECUTABLE_SUFFIXES
        .iter()
        .any(|allowed| name == *allowed || name.ends_with(allowed))
}

/// Scans the project root for structural problems.
pub fn scan(
    project_root: &Path,
    qcfg: &crate::quality_config::QualityConfig,
) -> Vec<StructureIssue> {
    let mut issues = Vec::new();
    let entries = match std::fs::read_dir(project_root) {
        Ok(e) => e,
        Err(e) => {
            return vec![StructureIssue {
                path: String::new(),
                rule: "read-root".to_string(),
                message: format!("failed to read project root: {e}"),
            }]
        }
    };
    for entry in entries.flatten() {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let path = project_root.join(&name);
        let is_symlink = std::fs::symlink_metadata(&path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        if is_symlink {
            continue;
        }
        let mode = entry
            .metadata()
            .map(|m| m.permissions().mode())
            .unwrap_or(0);
        let executable = mode & 0o111 != 0;
        if executable && !is_allowed_executable(&name) && !qcfg.should_ignore(&name, "structure") {
            issues.push(StructureIssue {
                path: name,
                rule: "root-executable".to_string(),
                message: "executable file in project root".to_string(),
            });
        }
    }
    issues.extend(check_required_files(project_root, qcfg));
    issues.extend(check_build_artifacts(project_root, qcfg));
    issues
}

/// Checks that the mandatory root files are present.
fn check_required_files(
    project_root: &Path,
    qcfg: &crate::quality_config::QualityConfig,
) -> Vec<StructureIssue> {
    let mut issues = Vec::new();
    for name in REQUIRED_FILES {
        if project_root.join(name).exists() || qcfg.should_ignore(name, "structure") {
            continue;
        }
        issues.push(StructureIssue {
            path: (*name).to_string(),
            rule: "missing-required-file".to_string(),
            message: "required file missing".to_string(),
        });
    }
    let has_manifest = MANIFEST_FILES
        .iter()
        .any(|name| project_root.join(name).exists());
    if !has_manifest {
        issues.push(StructureIssue {
            path: MANIFEST_FILES[0].to_string(),
            rule: "missing-required-file".to_string(),
            message: "no project manifest (go.mod or Cargo.toml)".to_string(),
        });
    }
    issues
}

/// Checks that build-artifact directories are git-ignored.
fn check_build_artifacts(
    project_root: &Path,
    qcfg: &crate::quality_config::QualityConfig,
) -> Vec<StructureIssue> {
    let gitignore = std::fs::read_to_string(project_root.join(".gitignore")).unwrap_or_default();
    BUILD_ARTIFACT_DIRS
        .iter()
        .filter(|artifact| project_root.join(artifact).is_dir())
        .filter(|artifact| !gitignore.contains(**artifact))
        .filter(|artifact| !qcfg.should_ignore(artifact, "structure"))
        .map(|artifact| StructureIssue {
            path: (*artifact).to_string(),
            rule: "build-artifact-untracked".to_string(),
            message: "build artifact directory not in .gitignore".to_string(),
        })
        .collect()
}

/// Runs the gate and returns its result.
pub fn run(cfg: &Config) -> Result<GateResult, String> {
    let qcfg = crate::quality_config::QualityConfig::load(cfg.root())?.unwrap_or_default();
    let issues = scan(cfg.root(), &qcfg);
    let mut report = Report::new("structure");
    for i in &issues {
        report.add(Diagnostic::error(
            i.path.clone(),
            i.rule.clone(),
            i.message.clone(),
        ));
    }
    if report.ok {
        report.summary = "structure: directory layout clean".to_string();
        Ok(GateResult::finish(report))
    } else {
        report.summary = format!("structure: {} issue(s)", issues.len());
        Ok(GateResult::failed(
            "structure",
            report.summary.clone(),
            report,
        ))
    }
}

/// The human-readable body printed when `--output` is not set.
pub fn text_body(result: &GateResult) -> String {
    let mut body = String::from("📁 Checking directory structure...\n");
    if result.passed() {
        body.push_str("✅ Directory structure is clean\n");
        return body;
    }
    for d in &result.report.diagnostics {
        body.push_str(&format!("  ⚠ {}: {}\n", d.path, d.message));
    }
    body.push_str(&format!(
        "structure check found {} issue(s)\n",
        result.report.diagnostics.len()
    ));
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_known_executable_names() {
        assert!(is_allowed_executable("Makefile"));
        assert!(is_allowed_executable("build.sh"));
        assert!(is_allowed_executable("Cargo.toml"));
        assert!(!is_allowed_executable("mystery-binary"));
    }

    #[test]
    fn accepts_either_manifest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "# x\n").unwrap();
        let qcfg = crate::quality_config::QualityConfig::default();
        assert!(!scan(dir.path(), &qcfg).is_empty());
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\n").unwrap();
        assert!(scan(dir.path(), &qcfg).is_empty());
    }
}
