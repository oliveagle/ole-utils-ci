// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The workflow topology: the released binary repo, its artifactory and the
//! FAAS apps that exercise it.
//!
//! Rust port of the Go original's `internal/workflow.go` data model and
//! `DefaultWorkflowConfig`. The built-in fallback is what lets `workflow`
//! work with no config file at all, so every field it sets is reproduced
//! here byte for byte: `workflow apps` prints it, and `workflow list` shows
//! it as the `builtin: yes` row.

use serde::{Deserialize, Serialize};

/// The user-level override file, below the repo-committed config.
pub const DEFAULT_WORKFLOW_CONFIG_PATH: &str = "~/.devine/utils/workflow.yaml";

/// The built-in workflow's name; it always resolves, even with no config.
pub const DEFAULT_WORKFLOW_NAME: &str = "devine-golang-mcp-app";

/// The role of the app that gives the authoritative rc smoke signal.
pub const ROLE_DEV_TEST: &str = "dev-test";

/// The role of the apps that exercise a production-like environment.
pub const ROLE_INTEGRATION: &str = "integration";

/// One Captain release group an app deploys to in PROD.
///
/// `enabled == false` means the group is suspended (for example SGP while the
/// public-cloud image sync is pending), and `deploy-prod` skips it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProdGroup {
    /// The Captain release-group id.
    #[serde(default)]
    pub id: i64,
    /// The cluster name (`SHAXY`, `SHA-ALI`, `SGP`).
    #[serde(default)]
    pub name: String,
    /// Whether `deploy-prod` publishes to this group.
    #[serde(default)]
    pub enabled: bool,
}

/// One FAAS app that exercises the binary runtime.
///
/// Test apps run CI on `main`; their `install-server.sh` resolves release
/// candidates from UAT and stable releases from PROD, and the app's
/// `/version` endpoint reports which binary it loaded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestApp {
    /// The catalog name (`devine-test-004-function`).
    #[serde(default)]
    pub name: String,
    /// The GitLab project path (`faas/devine-test-004-function`).
    #[serde(default, rename = "project")]
    pub project_path: String,
    /// One of [`ROLE_DEV_TEST`], [`ROLE_INTEGRATION`], or `playground`.
    #[serde(default)]
    pub role: String,
    /// A human description of what the app is for.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Whether the app's debug endpoints are open.
    #[serde(default)]
    pub debug: bool,
    /// Whether the app may carry dirty QConfig keys from prior versions.
    #[serde(default, skip_serializing_if = "is_false")]
    pub polluted: bool,
    /// The FAAS application id.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app_id: String,
    /// The `/version` endpoint the app publishes.
    #[serde(default)]
    pub version_url: String,
    /// The Captain FAT release-group id the image is published to.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fat_group: i64,
    /// The Captain PROD release groups.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prod_groups: Vec<ProdGroup>,
    /// The PROD base URL the `/spec` drift guard reads.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prod_url: String,
    /// The MCP tool that must exist in PROD `/spec` after a deploy.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub spec_tool: String,
}

impl TestApp {
    /// The number of PROD groups the app publishes to.
    pub fn enabled_groups(&self) -> usize {
        self.prod_groups.iter().filter(|g| g.enabled).count()
    }
}

/// The binary runtime repo that gets released.
///
/// The workflow commands operate on this repo (git bump + push); the test
/// apps consume the resulting artifactory artifacts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BinaryRepo {
    /// The repository name (`devine-golang-mcp-app`).
    pub name: String,
    /// The GitLab project path (`devine/devine-golang-mcp-app`).
    #[serde(rename = "project")]
    pub project_path: String,
    /// The browsable GitLab URL of the project.
    pub gitlab_base: String,
    /// The file holding the bare version (`VERSION`).
    pub version_file: String,
    /// The changelog the rc marker is inserted into (`CHANGELOG.md`).
    pub changelog_file: String,
    /// Files that mirror `version_file` (`pkg/version/VERSION`).
    pub mirror_files: Vec<String>,
    /// The git remote name (`origin`).
    pub remote: String,
}

/// The UAT and PROD artifact paths for the binary.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Artifactory {
    /// The artifactory base URL.
    pub base_url: String,
    /// The UAT repository path.
    pub uat_path: String,
    /// The PROD repository path.
    #[serde(rename = "prod_path")]
    pub prod_path: String,
    /// The smallest plausible artifact, in bytes.
    pub min_size_b: i64,
}

/// One named workflow: a binary repo, its FAAS test apps and the artifactory
/// paths they consume.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct WorkflowConfig {
    /// The registry key this workflow is stored under.
    pub name: String,
    /// The released binary repo.
    pub binary_repo: BinaryRepo,
    /// The UAT and PROD artifact paths.
    pub artifactory: Artifactory,
    /// The apps that exercise the binary.
    pub test_apps: Vec<TestApp>,
}

impl WorkflowConfig {
    /// Returns the catalog entry for a role or a name.
    ///
    /// An empty string returns the dev-test app (the authoritative rc
    /// smoke signal); otherwise the first entry whose name or role matches.
    pub fn find_test_app(&self, role_or_name: &str) -> Result<&TestApp, String> {
        if role_or_name.is_empty() {
            return self
                .test_apps
                .iter()
                .find(|app| app.role == ROLE_DEV_TEST)
                .ok_or_else(|| "workflow: no dev-test app in catalog".to_string());
        }
        self.test_apps
            .iter()
            .find(|app| app.name == role_or_name || app.role == role_or_name)
            .ok_or_else(|| {
                format!(
                    "workflow: no test app matching {role_or_name:?} (known: {})",
                    join_test_app_names(&self.test_apps)
                )
            })
    }

    /// Backfills the zero-valued fields after a merge.
    ///
    /// The Go original only restores these three: everything else a partial
    /// override leaves blank stays blank on purpose.
    pub fn apply_defaults(&mut self) {
        let def = super::defaults::default_workflow_config();
        if self.binary_repo.remote.is_empty() {
            self.binary_repo.remote = def.binary_repo.remote;
        }
        if self.binary_repo.version_file.is_empty() {
            self.binary_repo.version_file = def.binary_repo.version_file;
        }
        if self.artifactory.min_size_b == 0 {
            self.artifactory.min_size_b = def.artifactory.min_size_b;
        }
    }
}

/// `devine-test-004-function(dev-test), ...` — the spelling the miss error uses.
pub fn join_test_app_names(apps: &[TestApp]) -> String {
    apps.iter()
        .map(|app| format!("{}({})", app.name, app.role))
        .collect::<Vec<_>>()
        .join(", ")
}

/// One `workflows:` entry of a config file. Every field is optional, so a
/// repo file may restate the whole topology or tweak a single field.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkflowFileEntry {
    /// The binary repo half of the topology.
    #[serde(default)]
    pub binary_repo: BinaryRepo,
    /// The artifactory half of the topology.
    #[serde(default)]
    pub artifactory: Artifactory,
    /// The test app catalog.
    #[serde(default)]
    pub test_apps: Vec<TestApp>,
}

impl WorkflowFileEntry {
    /// Merges the non-zero fields of this entry onto `dst`.
    ///
    /// The Go original merges field by field and skips zero values, so an
    /// explicit `false` / `0` / `""` never clears a default. `changelog_file`
    /// is deliberately not merged (the Go merge omits it); a brand-new
    /// workflow still keeps whatever the file declared.
    pub fn merge_onto(&self, dst: &mut WorkflowConfig) {
        let repo = &self.binary_repo;
        let target = &mut dst.binary_repo;
        assign(&mut target.name, &repo.name);
        assign(&mut target.project_path, &repo.project_path);
        assign(&mut target.gitlab_base, &repo.gitlab_base);
        assign(&mut target.version_file, &repo.version_file);
        if !repo.mirror_files.is_empty() {
            target.mirror_files = repo.mirror_files.clone();
        }
        assign(&mut target.remote, &repo.remote);
        let art = &self.artifactory;
        let target = &mut dst.artifactory;
        assign(&mut target.base_url, &art.base_url);
        assign(&mut target.uat_path, &art.uat_path);
        assign(&mut target.prod_path, &art.prod_path);
        if art.min_size_b != 0 {
            target.min_size_b = art.min_size_b;
        }
        if !self.test_apps.is_empty() {
            dst.test_apps = self.test_apps.clone();
        }
    }

    /// Builds a whole workflow from this entry, for a name that is new.
    pub fn into_config(self, name: &str) -> WorkflowConfig {
        let mut cfg = WorkflowConfig {
            name: name.to_string(),
            binary_repo: self.binary_repo,
            artifactory: self.artifactory,
            test_apps: self.test_apps,
        };
        cfg.apply_defaults();
        cfg
    }
}

/// Copies `src` into `dst` when it carries a value.
fn assign(dst: &mut String, src: &str) {
    if !src.is_empty() {
        *dst = src.to_string();
    }
}

/// Serde helper: `json:"polluted,omitempty"`.
fn is_false(value: &bool) -> bool {
    !*value
}

/// Serde helper: `json:"fat_group,omitempty"`.
fn is_zero(value: &i64) -> bool {
    *value == 0
}

#[cfg(test)]
mod tests;
