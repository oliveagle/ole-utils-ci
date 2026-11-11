// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `lint:` block of `.devine-utils-ci.yaml`.
//!
//! The focused gates take their defaults from this file: the Go toolchain
//! pin's four surfaces, the directories the shell scan walks and the plugin
//! source tree. The file is optional — an absent file is the default policy,
//! not an error — and unknown keys are ignored so a newer project config
//! still loads.

use std::path::Path;

use serde::Deserialize;

/// The per-project configuration file the focused gates read.
pub const PROJECT_CONFIG_FILE: &str = ".devine-utils-ci.yaml";

/// The whole `.devine-utils-ci.yaml` document.
#[derive(Debug, Default, Deserialize)]
pub struct ProjectConfig {
    /// The `lint:` block.
    #[serde(default)]
    pub lint: LintConfig,
}

/// The `lint:` block.
#[derive(Debug, Default, Deserialize)]
pub struct LintConfig {
    /// The Go toolchain drift policy.
    #[serde(default, rename = "go_pin")]
    pub go_pin: GoPinConfig,
    /// The production shell-execution policy.
    #[serde(default, rename = "no_shell_exec")]
    pub no_shell_exec: NoShellExecConfig,
    /// The built-plugin freshness policy.
    #[serde(default, rename = "plugin_freshness")]
    pub plugin_freshness: PluginFreshnessConfig,
}

/// `lint.go_pin`.
#[derive(Debug, Default, Deserialize)]
pub struct GoPinConfig {
    /// Explicit on/off switch; `None` keeps the auto-detection.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// The single source of truth for the Go version.
    #[serde(default, rename = "version_file")]
    pub version_file: String,
    /// The GitLab pipeline file.
    #[serde(default, rename = "ci_file")]
    pub ci_file: String,
    /// The container build file; empty skips it.
    #[serde(default)]
    pub dockerfile: String,
    /// The script that pins the version for `require-go`.
    #[serde(default, rename = "require_go_script")]
    pub require_go_script: String,
    /// Also compare the running `go version`.
    #[serde(default, rename = "with_toolchain")]
    pub with_toolchain: bool,
}

/// `lint.no_shell_exec`.
#[derive(Debug, Default, Deserialize)]
pub struct NoShellExecConfig {
    /// Explicit on/off switch; `None` keeps the legacy default (off).
    #[serde(default)]
    pub enabled: Option<bool>,
    /// The production directories to scan.
    #[serde(default)]
    pub dirs: Vec<String>,
    /// Shell files that must not exist at all.
    #[serde(default, rename = "forbidden_paths")]
    pub forbidden_paths: Vec<String>,
}

/// `lint.plugin_freshness`.
#[derive(Debug, Default, Deserialize)]
pub struct PluginFreshnessConfig {
    /// Explicit on/off switch; `None` keeps the auto-detection.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// The plugin source root.
    #[serde(default, rename = "source_dir")]
    pub source_dir: String,
    /// The built plugin root.
    #[serde(default, rename = "plugins_dir")]
    pub plugins_dir: String,
}

/// The effective policy after defaults and auto-detection.
#[derive(Debug)]
pub struct Resolved {
    /// The toolchain pin surfaces.
    pub go_pin: GoPin,
    /// The shell scan directories.
    pub no_shell: NoShell,
    /// The plugin trees.
    pub plugin: Plugin,
}

/// The resolved toolchain pin policy.
#[derive(Debug)]
pub struct GoPin {
    /// The version source file.
    pub version_file: String,
    /// The GitLab pipeline file.
    pub ci_file: String,
    /// The container build file; empty skips the check.
    pub dockerfile: String,
    /// The `require-go` script; empty skips the check.
    pub require_go_script: String,
    /// Whether the running toolchain must match too.
    pub with_toolchain: bool,
}

/// The resolved shell scan policy.
#[derive(Debug)]
pub struct NoShell {
    /// The production directories to scan.
    pub dirs: Vec<String>,
    /// Shell files that must not exist at all.
    pub forbidden_paths: Vec<String>,
}

/// The resolved plugin freshness policy.
#[derive(Debug)]
pub struct Plugin {
    /// The plugin source root.
    pub source_dir: String,
    /// The built plugin root.
    pub plugins_dir: String,
}

/// Reads `.devine-utils-ci.yaml` from `root`.
///
/// A missing file is the default policy; a file that cannot be read or
/// parsed is an error, worded the way the Go original words it.
pub fn load(root: &Path) -> Result<ProjectConfig, String> {
    let path = root.join(PROJECT_CONFIG_FILE);
    let data = match std::fs::read_to_string(&path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ProjectConfig::default()),
        Err(e) => return Err(format!("read {PROJECT_CONFIG_FILE}: {e}")),
    };
    serde_yaml::from_str(&data).map_err(|e| format!("parse {PROJECT_CONFIG_FILE}: {e}"))
}

/// Applies the defaults the Go original applies to one project.
pub fn resolve(config: &ProjectConfig) -> Resolved {
    let lint = &config.lint;
    let go_pin = GoPin {
        version_file: first_non_empty(&lint.go_pin.version_file, ".go-version"),
        ci_file: first_non_empty(&lint.go_pin.ci_file, ".gitlab-ci.yml"),
        dockerfile: first_non_empty(&lint.go_pin.dockerfile, "Dockerfile"),
        require_go_script: first_non_empty(
            &lint.go_pin.require_go_script,
            "scripts/ci/require-go.sh",
        ),
        with_toolchain: lint.go_pin.with_toolchain,
    };
    let mut dirs = lint.no_shell_exec.dirs.clone();
    if dirs.is_empty() {
        dirs = vec!["cmd".to_string(), "internal".to_string()];
    }
    let source_dir = first_non_empty(&lint.plugin_freshness.source_dir, "src/golang");
    let plugins_dir = first_non_empty(&lint.plugin_freshness.plugins_dir, "plugins");
    Resolved {
        go_pin,
        no_shell: NoShell {
            dirs,
            forbidden_paths: lint.no_shell_exec.forbidden_paths.clone(),
        },
        plugin: Plugin {
            source_dir,
            plugins_dir,
        },
    }
}

/// The first non-empty value, or `fallback`.
fn first_non_empty(value: &str, fallback: &str) -> String {
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_is_the_default_policy() {
        let config = load(Path::new("/nonexistent-config-for-tests")).expect("load");
        let resolved = resolve(&config);
        assert_eq!(resolved.go_pin.version_file, ".go-version");
        assert_eq!(resolved.no_shell.dirs, vec!["cmd", "internal"]);
        assert_eq!(resolved.plugin.source_dir, "src/golang");
    }

    #[test]
    fn the_lint_block_is_read() {
        let config: ProjectConfig = serde_yaml::from_str(
            "lint:\n  go_pin:\n    version_file: VERSION.go\n  no_shell_exec:\n    dirs: [a]\n",
        )
        .expect("parse");
        let resolved = resolve(&config);
        assert_eq!(resolved.go_pin.version_file, "VERSION.go");
        assert_eq!(resolved.no_shell.dirs, vec!["a"]);
    }
}
