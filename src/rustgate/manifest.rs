// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Manifest hygiene for `Cargo.toml`.
//!
//! These rules catch the dependency mistakes that are easy to make and
//! expensive to discover later: a wildcard version, a git dependency without
//! a pinned revision, a crate that is both a normal and a dev dependency, or
//! a release profile that ships debug symbols.

use std::collections::BTreeMap;
use std::path::Path;

use toml::Value;

/// One manifest problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestFinding {
    /// The key the problem is attached to.
    pub key: String,
    /// The rule that was broken.
    pub rule: String,
    /// A one-line description.
    pub message: String,
}

/// The parsed facts about a manifest.
#[derive(Debug, Clone, Default)]
pub struct ManifestFacts {
    /// The package name.
    pub name: String,
    /// The package version.
    pub version: String,
    /// The declared minimum supported Rust version.
    pub rust_version: String,
    /// The declared licence.
    pub license: String,
    /// The number of normal dependencies.
    pub dependency_count: usize,
    /// The number of dev-dependencies.
    pub dev_dependency_count: usize,
    /// Whether the release profile strips symbols.
    pub strips_symbols: bool,
    /// Whether the release profile enables link-time optimisation.
    pub lto: bool,
}

/// Reads and analyses the manifest in `root`.
pub fn analyze(root: &Path) -> Result<(ManifestFacts, Vec<ManifestFinding>), String> {
    let path = root.join("Cargo.toml");
    let raw =
        std::fs::read_to_string(&path).map_err(|e| format!("failed to read Cargo.toml: {e}"))?;
    let value: Value =
        toml::from_str(&raw).map_err(|e| format!("failed to parse Cargo.toml: {e}"))?;
    let mut findings = Vec::new();
    let package = value.get("package");
    let facts = collect_facts(package, &value);
    findings.extend(check_package(&facts));
    findings.extend(check_dependencies(&value));
    findings.extend(check_release_profile(&facts));
    Ok((facts, findings))
}

fn collect_facts(package: Option<&Value>, root: &Value) -> ManifestFacts {
    let mut facts = ManifestFacts::default();
    if let Some(pkg) = package {
        facts.name = pkg
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        facts.version = pkg
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        facts.rust_version = pkg
            .get("rust-version")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        facts.license = pkg
            .get("license")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
    }
    facts.dependency_count = table_len(root.get("dependencies"));
    facts.dev_dependency_count = table_len(root.get("dev-dependencies"));
    if let Some(profile) = root.get("profile").and_then(|p| p.get("release")) {
        facts.strips_symbols = profile
            .get("strip")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        facts.lto = match profile.get("lto") {
            Some(Value::Boolean(true)) => true,
            Some(Value::String(s)) => s != "false" && s != "off",
            _ => false,
        };
    }
    facts
}

fn table_len(value: Option<&Value>) -> usize {
    match value {
        Some(Value::Table(t)) => t.len(),
        _ => 0,
    }
}

fn check_package(facts: &ManifestFacts) -> Vec<ManifestFinding> {
    let mut findings = Vec::new();
    if facts.name.is_empty() {
        findings.push(ManifestFinding {
            key: "package.name".to_string(),
            rule: "rust/manifest-name".to_string(),
            message: "the package has no name".to_string(),
        });
    }
    if facts.license.is_empty() {
        findings.push(ManifestFinding {
            key: "package.license".to_string(),
            rule: "rust/manifest-license".to_string(),
            message: "the package declares no licence".to_string(),
        });
    }
    if facts.rust_version.is_empty() {
        findings.push(ManifestFinding {
            key: "package.rust-version".to_string(),
            rule: "rust/manifest-msrv".to_string(),
            message: "the package declares no rust-version (MSRV)".to_string(),
        });
    }
    findings
}

fn check_dependencies(root: &Value) -> Vec<ManifestFinding> {
    let mut findings = Vec::new();
    let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(table) = root.get(section).and_then(|v| v.as_table()) else {
            continue;
        };
        for (name, spec) in table {
            check_dependency_spec(name, spec, section, &mut findings);
            seen.entry(name.clone())
                .or_default()
                .push(section.to_string());
        }
    }
    for (name, sections) in &seen {
        if sections.len() > 1 && sections.contains(&"dependencies".to_string()) {
            findings.push(ManifestFinding {
                key: format!("dependencies.{name}"),
                rule: "rust/dependency-duplicate".to_string(),
                message: format!("{name} is declared as both a normal and a build/dev dependency"),
            });
        }
    }
    findings
}

fn check_dependency_spec(
    name: &str,
    spec: &Value,
    section: &str,
    findings: &mut Vec<ManifestFinding>,
) {
    // A bare `name = "1.2"` dependency is the version requirement itself.
    if let Some(direct) = spec.as_str() {
        check_version_requirement(name, direct, section, findings);
        return;
    }
    let version = spec.get("version").and_then(|v| v.as_str()).unwrap_or("");
    check_version_requirement(name, version, section, findings);
    if spec.get("git").is_some() && spec.get("rev").is_none() && spec.get("tag").is_none() {
        findings.push(ManifestFinding {
            key: format!("{section}.{name}.git"),
            rule: "rust/dependency-unpinned-git".to_string(),
            message: format!("{name} comes from git without a pinned rev or tag"),
        });
    }
    if spec.get("path").is_some() && spec.get("version").is_none() {
        findings.push(ManifestFinding {
            key: format!("{section}.{name}.path"),
            rule: "rust/dependency-path".to_string(),
            message: format!("{name} is a path dependency and is not publishable"),
        });
    }
}

/// Reports wildcard version requirements.
fn check_version_requirement(
    name: &str,
    version: &str,
    section: &str,
    findings: &mut Vec<ManifestFinding>,
) {
    if version == "*" {
        findings.push(ManifestFinding {
            key: format!("{section}.{name}.version"),
            rule: "rust/dependency-wildcard".to_string(),
            message: format!("{name} uses a wildcard version"),
        });
        return;
    }
    if version.contains('*') {
        findings.push(ManifestFinding {
            key: format!("{section}.{name}.version"),
            rule: "rust/dependency-partial-wildcard".to_string(),
            message: format!("{name} uses a partial wildcard version {version:?}"),
        });
    }
}

fn check_release_profile(facts: &ManifestFacts) -> Vec<ManifestFinding> {
    let mut findings = Vec::new();
    if !facts.strips_symbols {
        findings.push(ManifestFinding {
            key: "profile.release.strip".to_string(),
            rule: "rust/manifest-strip".to_string(),
            message: "the release profile does not strip symbols".to_string(),
        });
    }
    if !facts.lto {
        findings.push(ManifestFinding {
            key: "profile.release.lto".to_string(),
            rule: "rust/manifest-lto".to_string(),
            message: "the release profile does not enable link-time optimisation".to_string(),
        });
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_wildcard_and_unpinned_dependencies() {
        let raw = r#"
[package]
name = "x"
version = "0.1.0"

[dependencies]
serde = "*"
other = { git = "https://example.com/o" }
"#;
        let value: Value = toml::from_str(raw).unwrap();
        let mut findings = Vec::new();
        check_dependency_spec(
            "serde",
            &value["dependencies"]["serde"],
            "dependencies",
            &mut findings,
        );
        check_dependency_spec(
            "other",
            &value["dependencies"]["other"],
            "dependencies",
            &mut findings,
        );
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn reads_package_facts() {
        let raw = "[package]\nname = \"x\"\nversion = \"1.0.0\"\nlicense = \"MIT\"\nrust-version = \"1.85\"\n";
        let value: Value = toml::from_str(raw).unwrap();
        let facts = collect_facts(value.get("package"), &value);
        assert_eq!(facts.name, "x");
        assert_eq!(facts.license, "MIT");
        assert_eq!(facts.rust_version, "1.85");
    }
}
