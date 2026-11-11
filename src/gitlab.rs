// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The GitLab-facing helpers the networked commands share.
//!
//! `pipeline-run`, `pipeline-watch` and `workflow` all need the same three
//! things: a token resolved through the documented fallback chain, the
//! YAML config file that names the instance and the watched repos, and
//! `url.PathEscape` for the project paths they put in API URLs. The Go
//! original keeps these in `internal/`; this module is their Rust home.

use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use crate::runner::Runner;

/// The default GitLab instance every command falls back to.
pub const DEFAULT_BASE_URL: &str = "https://git.dev.sh.ctripcorp.com";

/// Runs `dotvault get <key>`, trimmed, exactly the way the Go original
/// shells out to the keyring helper.
pub fn dotvault_get(key: &str) -> Result<String, String> {
    let out = Runner::new(".", false).run("dotvault", &["get", key])?;
    Ok(out.trim().to_string())
}

/// The three-tier token source: config literal → environment → dotvault.
#[derive(Debug, Clone)]
pub struct TokenSource {
    /// A token written literally into the YAML config (may be empty).
    config_token: String,
    /// The environment variable tier, also the hint in every error.
    env_var: String,
    /// The dotvault key of the third tier.
    dotvault_key: String,
}

impl TokenSource {
    /// The defaults matching the pipeline-watch PRD.
    pub fn new(config_token: &str) -> Self {
        TokenSource {
            config_token: config_token.to_string(),
            env_var: "DEVINE_GIT_GROUP_ACCESS_TOKEN".to_string(),
            dotvault_key: "DEVINE_GIT_GROUP_ACCESS_KEY".to_string(),
        }
    }

    /// Resolves the first non-empty tier, returning the token and where it
    /// came from (`config`, `env:NAME`, `dotvault:KEY`).
    pub fn resolve(&self) -> Result<(String, String), String> {
        let from_config = self.config_token.trim();
        if !from_config.is_empty() {
            return Ok((from_config.to_string(), "config".to_string()));
        }
        if !self.env_var.is_empty() {
            if let Ok(value) = std::env::var(&self.env_var) {
                let value = value.trim();
                if !value.is_empty() {
                    return Ok((value.to_string(), format!("env:{}", self.env_var)));
                }
            }
        }
        if self.dotvault_key.is_empty() {
            return Ok((String::new(), String::new()));
        }
        if !Runner::command_exists("dotvault") {
            return Err(format!(
                "dotvault not on PATH (set {} env or install dotvault)",
                self.env_var
            ));
        }
        let runner = Runner::new(".", false);
        let output = runner.run_with_timeout(
            Duration::from_secs(5),
            "dotvault",
            &["get", &self.dotvault_key],
        )?;
        if !output.success {
            let detail = output.output.trim();
            if detail.is_empty() {
                return Err(format!("dotvault get {} failed", self.dotvault_key));
            }
            return Err(format!("dotvault get {}: {}", self.dotvault_key, detail));
        }
        let token = output.output.trim().to_string();
        if token.is_empty() {
            return Ok((String::new(), String::new()));
        }
        Ok((token, format!("dotvault:{}", self.dotvault_key)))
    }
}

/// `pipeline-run`'s token chain: `--token` → `$GITLAB_TOKEN` →
/// `$DEVINE_GIT_GROUP_ACCESS_TOKEN` → dotvault.
///
/// Every failure collapses to one message, the way the Go original does.
pub fn resolve_pipeline_run_token(explicit: &str) -> Result<(String, String), String> {
    let explicit = explicit.trim();
    if !explicit.is_empty() {
        return Ok((explicit.to_string(), "--token flag".to_string()));
    }
    for (variable, label) in [
        ("GITLAB_TOKEN", "env:GITLAB_TOKEN"),
        (
            "DEVINE_GIT_GROUP_ACCESS_TOKEN",
            "env:DEVINE_GIT_GROUP_ACCESS_TOKEN",
        ),
    ] {
        if let Ok(value) = std::env::var(variable) {
            let value = value.trim();
            if !value.is_empty() {
                return Ok((value.to_string(), label.to_string()));
            }
        }
    }
    let missing = "no token found: set --token flag, $GITLAB_TOKEN env, or configure dotvault (DEVINE_GIT_GROUP_ACCESS_KEY)";
    match TokenSource::new("").resolve() {
        Ok((token, where_)) if !token.is_empty() => Ok((token, where_)),
        _ => Err(missing.to_string()),
    }
}

/// One watched repository: either a whole group or a single project.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WatchRepo {
    /// A GitLab group path whose projects are watched as a whole.
    #[serde(default)]
    pub group: String,
    /// One project path, used when `group` is empty.
    #[serde(default)]
    pub project: String,
}

/// The `refresh:` sub-block of the watch config.
#[derive(Debug, Clone, Deserialize)]
pub struct WatchRefresh {
    /// Seconds between refreshes; clamped into `[5, 120]`.
    #[serde(default = "default_interval")]
    pub interval_seconds: i64,
}

fn default_interval() -> i64 {
    10
}

impl Default for WatchRefresh {
    fn default() -> Self {
        WatchRefresh {
            interval_seconds: default_interval(),
        }
    }
}

fn default_per_repo() -> i64 {
    7
}

/// The pipeline-watch / pipeline-run YAML config.
///
/// The schema is intentionally minimal: instance URL, optional literal
/// token, refresh cadence, window size and the watched repos.
#[derive(Debug, Clone, Deserialize)]
pub struct WatchConfig {
    /// GitLab base URL without a trailing slash.
    #[serde(default = "default_base_url")]
    pub base_url: String,
    /// Optional literal token; falls back to env / dotvault.
    #[serde(default)]
    pub token: String,
    /// Auto-refresh settings.
    #[serde(default)]
    pub refresh: WatchRefresh,
    /// How many pipelines each repo row shows.
    #[serde(default = "default_per_repo")]
    pub per_repo: i64,
    /// Local clones used for the ahead/behind column.
    #[serde(default)]
    pub local_root: String,
    /// The watched groups and projects.
    #[serde(default)]
    pub repos: Vec<WatchRepo>,
}

fn default_base_url() -> String {
    DEFAULT_BASE_URL.to_string()
}

impl Default for WatchConfig {
    fn default() -> Self {
        WatchConfig {
            base_url: default_base_url(),
            token: String::new(),
            refresh: WatchRefresh {
                interval_seconds: default_interval(),
            },
            per_repo: default_per_repo(),
            local_root: String::new(),
            repos: Vec::new(),
        }
    }
}

impl WatchConfig {
    /// Backfills the zero-valued fields the way the Go original does after
    /// unmarshalling: blank URL, clamped interval, clamped window.
    pub fn apply_defaults(&mut self) {
        if self.base_url.trim().is_empty() {
            self.base_url = default_base_url();
        }
        self.refresh.interval_seconds = self.refresh.interval_seconds.clamp(5, 120);
        if self.per_repo <= 0 {
            self.per_repo = default_per_repo();
        }
        if self.per_repo > 20 {
            self.per_repo = 20;
        }
    }
}

/// The conventional config location: `$HOME/.devine/utils/ci/config.yaml`.
pub fn default_config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    if home.is_empty() {
        return None;
    }
    Some(PathBuf::from(home).join(".devine/utils/ci/config.yaml"))
}

/// Reads the watch config. A missing file is a soft miss (defaults win); a
/// malformed one is a hard error, matching `LoadPipelineWatchConfig`.
pub fn load_watch_config(path: Option<&str>) -> Result<WatchConfig, String> {
    let resolved: PathBuf = match path {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => match default_config_path() {
            Some(p) => p,
            None => return Ok(WatchConfig::default()),
        },
    };
    let data = match std::fs::read_to_string(&resolved) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WatchConfig::default())
        }
        Err(error) => return Err(format!("read config {}: {error}", resolved.display())),
    };
    if data.is_empty() {
        return Ok(WatchConfig::default());
    }
    let display = resolved.display().to_string();
    let mut config: WatchConfig =
        serde_yaml::from_str(&data).map_err(|error| format!("parse config {display}: {error}"))?;
    config.apply_defaults();
    Ok(config)
}

/// `url.PathEscape`: percent-encodes everything outside the path-segment
/// keep set, using upper-case hex over the UTF-8 bytes.
///
/// The keep set is Go's `shouldEscape` answer for `encodePathSegment`,
/// verified against `net/url`: alphanumerics plus `- _ . ~ $ & + : = @`.
pub fn path_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        let keep = byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_' | b'.' | b'~' | b'$' | b'&' | b'+' | b':' | b'=' | b'@'
            );
        if keep {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_escape_matches_net_url() {
        let cases = [
            (
                "v[0-9]*.[0-9]*.[0-9]-*",
                "v%5B0-9%5D%2A.%5B0-9%5D%2A.%5B0-9%5D-%2A",
            ),
            ("v*", "v%2A"),
            ("release", "release"),
            ("release/*", "release%2F%2A"),
            (
                "faas/devine-app-integration-01-function",
                "faas%2Fdevine-app-integration-01-function",
            ),
            ("a b/c?d#e%f*g(h)i", "a%20b%2Fc%3Fd%23e%25f%2Ag%28h%29i"),
        ];
        for (input, expected) in cases {
            assert_eq!(path_escape(input), expected, "{input}");
        }
    }

    #[test]
    fn watch_defaults_survive_a_sparse_config() {
        let config: WatchConfig = serde_yaml::from_str("repos:\n  - group: devine\n").unwrap();
        let mut config = config;
        config.apply_defaults();
        assert_eq!(config.base_url, DEFAULT_BASE_URL);
        assert_eq!(config.refresh.interval_seconds, 10);
        assert_eq!(config.per_repo, 7);
        assert_eq!(config.repos.len(), 1);
        assert_eq!(config.repos[0].group, "devine");
    }

    #[test]
    fn watch_interval_is_clamped_into_the_documented_window() {
        let mut config: WatchConfig =
            serde_yaml::from_str("refresh:\n  interval_seconds: 1\n").unwrap();
        config.apply_defaults();
        assert_eq!(config.refresh.interval_seconds, 5);
        let mut config: WatchConfig =
            serde_yaml::from_str("refresh:\n  interval_seconds: 999\n").unwrap();
        config.apply_defaults();
        assert_eq!(config.refresh.interval_seconds, 120);
    }
}
