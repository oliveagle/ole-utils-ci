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
//!
//! [`resolve`] applies the defaults the Go original applies: the four
//! formatter switches default to on, `go_pin` switches itself on when a
//! version file exists, `plugin_freshness` when both of its trees exist, and
//! the shell scan and the web pass stay off until a project asks for them.

mod project;
mod web;

pub use project::{
    GoPinConfig, LintConfig, NoShellExecConfig, PluginFreshnessConfig, ProjectConfig, TreeMirror,
};
pub use web::{ResolvedWeb, WebConfig};

use std::path::Path;

/// The per-project configuration file the focused gates read.
pub const PROJECT_CONFIG_FILE: &str = ".devine-utils-ci.yaml";

/// The Go trees the formatter walks when the project names none.
const DEFAULT_FIRST_PARTY_DIRS: [&str; 7] = [
    "cmd",
    "internal",
    "pkg",
    "sandbox",
    "src",
    "tests",
    "third_party",
];

/// The effective policy after defaults and auto-detection.
#[derive(Debug)]
pub struct Resolved {
    /// The Go trees the formatter walks.
    pub first_party_dirs: Vec<String>,
    /// Whether `gofmt -s` runs.
    pub go_format: bool,
    /// Whether `go mod edit -fmt` runs.
    pub go_mod_format: bool,
    /// Whether `go vet` runs.
    pub go_vet: bool,
    /// Whether `golangci-lint` runs.
    pub golangci_lint: bool,
    /// The toolchain pin surfaces.
    pub go_pin: GoPin,
    /// The shell scan directories.
    pub no_shell: NoShell,
    /// The plugin trees.
    pub plugin: Plugin,
    /// The front-end syntax policy.
    pub web: ResolvedWeb,
}

/// The resolved toolchain pin policy.
#[derive(Debug)]
pub struct GoPin {
    /// Whether the gate runs at all.
    pub enabled: bool,
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
    /// Whether the gate runs at all.
    pub enabled: bool,
    /// The production directories to scan.
    pub dirs: Vec<String>,
    /// Shell files that must not exist at all.
    pub forbidden_paths: Vec<String>,
}

/// The resolved plugin freshness policy.
#[derive(Debug)]
pub struct Plugin {
    /// Whether the gate runs at all.
    pub enabled: bool,
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
    serde_norway::from_str(&data).map_err(|e| format!("parse {PROJECT_CONFIG_FILE}: {e}"))
}

/// Applies the defaults the Go original applies to one project.
///
/// `root` is the repository the policy applies to: the auto-detected gates
/// look at its files to decide whether they apply at all.
pub fn resolve(root: &Path, config: &ProjectConfig) -> Resolved {
    let lint = &config.lint;
    let version_file = first_non_empty(&lint.go_pin.version_file, ".go-version");
    let go_pin = GoPin {
        enabled: bool_value(lint.go_pin.enabled, path_exists(root, &version_file)),
        version_file,
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
    let plugin = Plugin {
        enabled: bool_value(
            lint.plugin_freshness.enabled,
            path_exists(root, &source_dir) && path_exists(root, &plugins_dir),
        ),
        source_dir,
        plugins_dir,
    };
    Resolved {
        first_party_dirs: first_party_dirs(&lint.first_party_dirs),
        go_format: bool_value(lint.go_format, true),
        go_mod_format: bool_value(lint.go_mod_format, true),
        go_vet: bool_value(lint.go_vet, true),
        golangci_lint: bool_value(lint.golangci_lint, true),
        go_pin,
        no_shell: NoShell {
            enabled: bool_value(lint.no_shell_exec.enabled, false),
            dirs,
            forbidden_paths: lint.no_shell_exec.forbidden_paths.clone(),
        },
        plugin,
        web: resolve_web(&lint.web),
    }
}

/// The Go trees the formatter walks.
fn first_party_dirs(configured: &[String]) -> Vec<String> {
    if configured.is_empty() {
        return DEFAULT_FIRST_PARTY_DIRS
            .iter()
            .map(|dir| (*dir).to_string())
            .collect();
    }
    configured.to_vec()
}

/// The effective web-lint policy.
fn resolve_web(config: &WebConfig) -> ResolvedWeb {
    let enabled = bool_value(config.enabled, false);
    let mut paths = config.paths.clone();
    if enabled && paths.is_empty() {
        paths = vec![".".to_string()];
    }
    ResolvedWeb {
        enabled,
        paths,
        ts_engine: first_non_empty(&config.ts_engine, "auto"),
        tsconfig: config.tsconfig.clone(),
        ignore: config.ignore.clone(),
        max_files: config.max_files,
        max_file_size: config.max_file_size,
        timeout_seconds: config.timeout_seconds,
        fail_on: first_non_empty(&config.fail_on, "error"),
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

/// The configured value, or the fallback when the project names none.
fn bool_value(value: Option<bool>, fallback: bool) -> bool {
    value.unwrap_or(fallback)
}

/// Whether `rel` names something that exists under `root`.
fn path_exists(root: &Path, rel: &str) -> bool {
    root.join(rel).exists()
}

#[cfg(test)]
mod tests;
