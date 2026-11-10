// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `.devine-utils-ci.jsonc` project configuration.
//!
//! The format is JSON with comments (JSONC). A project may *tighten* the
//! built-in budgets here; it may never raise them, switch a file-length
//! profile, exclude a path from a hard budget, or use an unknown field. Each
//! of those is a configuration error, reported with a message that names the
//! offending key.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// JSONC comment stripping and the glob matcher behind the ignore rules.
pub mod glob;

pub use glob::{match_glob_pattern, match_package_pattern, strip_jsonc_comments};

/// A warning/error budget expressed in lines.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct QualityLineLimit {
    /// Warning threshold.
    #[serde(default)]
    pub warn: usize,
    /// Hard threshold.
    #[serde(default)]
    pub max: usize,
}

/// A per-path file-length rule. `mode` and `profile` stay decodable so an
/// old or unsafe config fails with a clear error instead of being ignored.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityFileLengthRule {
    /// Legacy key; setting it is an error.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mode: String,
    /// Legacy key; setting it is an error.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub profile: String,
    /// Warning threshold for matching paths.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub warn: usize,
    /// Hard threshold for matching paths.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub max: usize,
}

fn is_zero_usize(v: &usize) -> bool {
    *v == 0
}

/// File-length customisation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityFileLengthConfig {
    /// Per-profile budgets, e.g. `{"source": {"warn": 300, "max": 400}}`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<String, QualityLineLimit>,
    /// Per-path budgets, keyed by glob.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub paths: BTreeMap<String, QualityFileLengthRule>,
}

/// Complexity customisation. The CLI stays authoritative for the hard cap.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityComplexityConfig {
    /// Warning threshold.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub warn: usize,
    /// Hard threshold.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub max: usize,
}

/// Package-size budgets. All of them are warning-only.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityPackageSizeConfig {
    /// File-count warning threshold.
    #[serde(default, rename = "warn-files", skip_serializing_if = "is_zero_usize")]
    pub warn_files: usize,
    /// Line-count warning threshold.
    #[serde(default, rename = "warn-lines", skip_serializing_if = "is_zero_usize")]
    pub warn_lines: usize,
    /// Exported-symbol warning threshold.
    #[serde(
        default,
        rename = "warn-exported-symbols",
        skip_serializing_if = "is_zero_usize"
    )]
    pub warn_exported_symbols: usize,
    /// Import fan-out warning threshold.
    #[serde(
        default,
        rename = "warn-imports",
        skip_serializing_if = "is_zero_usize"
    )]
    pub warn_imports: usize,
}

/// One artifact-size rule.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityArtifactSizeRule {
    /// Glob matched against the artifact path.
    #[serde(default)]
    pub glob: String,
    /// Warning budget in MiB.
    #[serde(default, rename = "warn-mb", skip_serializing_if = "is_zero_f64")]
    pub warn_mb: f64,
    /// Hard budget in MiB.
    #[serde(default, rename = "max-mb", skip_serializing_if = "is_zero_f64")]
    pub max_mb: f64,
    /// Whether the artifact must exist.
    #[serde(default, skip_serializing_if = "is_false")]
    pub required: bool,
}

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// Artifact-size customisation. Inert until at least one path rule exists.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityArtifactSizeConfig {
    /// The rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<QualityArtifactSizeRule>,
}

/// The ignore section of one check.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityIgnoreSection {
    /// Glob patterns to skip.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

/// The coverage ignore section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityIgnoreCoverage {
    /// Package patterns allowed to sit below the threshold.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<String>,
}

/// All ignore patterns.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityIgnore {
    /// Patterns applied to every path-based check.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    /// Sanity-check ignores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sanity: Option<QualityIgnoreSection>,
    /// Bug-marker ignores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bugs: Option<QualityIgnoreSection>,
    /// File-length ignores (not allowed for hard budgets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_length: Option<QualityIgnoreSection>,
    /// Complexity ignores (not allowed for hard budgets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity: Option<QualityIgnoreSection>,
    /// License ignores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<QualityIgnoreSection>,
    /// Structure ignores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<QualityIgnoreSection>,
    /// Package-size ignores (not allowed for hard budgets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_size: Option<QualityIgnoreSection>,
    /// Artifact-size ignores (not allowed for hard budgets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_size: Option<QualityIgnoreSection>,
    /// Coverage package ignores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<QualityIgnoreCoverage>,
}

/// The whole `.devine-utils-ci.jsonc` document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualityConfig {
    /// Ignore patterns.
    #[serde(default)]
    pub ignore: QualityIgnore,
    /// File-length customisation.
    #[serde(
        default,
        rename = "file-length",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_length: Option<QualityFileLengthConfig>,
    /// Complexity customisation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity: Option<QualityComplexityConfig>,
    /// Package-size customisation.
    #[serde(
        default,
        rename = "package-size",
        skip_serializing_if = "Option::is_none"
    )]
    pub package_size: Option<QualityPackageSizeConfig>,
    /// Artifact-size customisation.
    #[serde(
        default,
        rename = "artifact-size",
        skip_serializing_if = "Option::is_none"
    )]
    pub artifact_size: Option<QualityArtifactSizeConfig>,
}

/// The name of the project configuration file.
pub const CONFIG_FILE: &str = ".devine-utils-ci.jsonc";

impl QualityConfig {
    /// Loads the configuration from `project_root`.
    ///
    /// Returns `Ok(None)` when the project has no configuration file.
    pub fn load(project_root: &Path) -> Result<Option<QualityConfig>, String> {
        let path = project_root.join(CONFIG_FILE);
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("failed to read quality config: {e}")),
        };
        let cleaned = strip_jsonc_comments(&data);
        let cfg: QualityConfig = serde_json::from_str(&cleaned)
            .map_err(|e| format!("failed to parse quality config: {e}"))?;
        Ok(Some(cfg))
    }

    /// Whether `path` should be ignored by `check`.
    pub fn should_ignore(&self, path: &str, check: &str) -> bool {
        let path = path.replace('\\', "/");
        if self
            .ignore
            .paths
            .iter()
            .any(|p| match_glob_pattern(&path, p))
        {
            return true;
        }
        let section = match check {
            "sanity" => &self.ignore.sanity,
            "bugs" => &self.ignore.bugs,
            "file-length" => &self.ignore.file_length,
            "complexity" => &self.ignore.complexity,
            "license" => &self.ignore.license,
            "structure" => &self.ignore.structure,
            "package-size" => &self.ignore.package_size,
            "artifact-size" => &self.ignore.artifact_size,
            _ => return false,
        };
        match section {
            Some(s) => s.paths.iter().any(|p| match_glob_pattern(&path, p)),
            None => false,
        }
    }

    /// Whether `pkg` may sit below the coverage threshold.
    pub fn should_ignore_package(&self, pkg: &str) -> bool {
        match &self.ignore.coverage {
            Some(c) => c.packages.iter().any(|p| match_package_pattern(pkg, p)),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_config_fields() {
        let err = serde_json::from_str::<QualityConfig>("{\"nope\": 1}").unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }
}
