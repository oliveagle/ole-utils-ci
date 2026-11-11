// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The data model of `.devine-utils-ci.yaml`.
//!
//! Every switch that the Go original spells as a pointer boolean is an
//! `Option<bool>` here: an absent switch keeps the legacy default, while an
//! explicit `false` always disables the check it names.

use serde::Deserialize;

use super::web::WebConfig;

/// The whole `.devine-utils-ci.yaml` document.
#[derive(Debug, Default, Deserialize)]
pub struct ProjectConfig {
    /// The `lint:` block.
    #[serde(default)]
    pub lint: LintConfig,
    /// Files that must carry the same content as the root `VERSION`.
    #[serde(default)]
    pub version_mirrors: Vec<String>,
    /// Directory trees that must stay byte-identical to their source.
    #[serde(default)]
    pub tree_mirrors: Vec<TreeMirror>,
}

/// One `tree_mirrors` entry: one source tree and its mirrors.
#[derive(Debug, Default, Deserialize)]
pub struct TreeMirror {
    /// The source directory, relative to the project root.
    #[serde(default)]
    pub src: String,
    /// The destination directories, relative to the project root.
    #[serde(default)]
    pub dsts: Vec<String>,
}

/// The `lint:` block.
#[derive(Debug, Default, Deserialize)]
pub struct LintConfig {
    /// The first-party Go trees the formatter walks.
    #[serde(default, rename = "first_party_dirs")]
    pub first_party_dirs: Vec<String>,
    /// Whether `gofmt -s` runs.
    #[serde(default, rename = "go_format")]
    pub go_format: Option<bool>,
    /// Whether `go mod edit -fmt` runs.
    #[serde(default, rename = "go_mod_format")]
    pub go_mod_format: Option<bool>,
    /// Whether `go vet` runs.
    #[serde(default, rename = "go_vet")]
    pub go_vet: Option<bool>,
    /// Whether `golangci-lint` runs.
    #[serde(default, rename = "golangci_lint")]
    pub golangci_lint: Option<bool>,
    /// The Go toolchain drift policy.
    #[serde(default, rename = "go_pin")]
    pub go_pin: GoPinConfig,
    /// The production shell-execution policy.
    #[serde(default, rename = "no_shell_exec")]
    pub no_shell_exec: NoShellExecConfig,
    /// The built-plugin freshness policy.
    #[serde(default, rename = "plugin_freshness")]
    pub plugin_freshness: PluginFreshnessConfig,
    /// The front-end syntax policy.
    #[serde(default)]
    pub web: WebConfig,
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
