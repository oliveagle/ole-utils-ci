// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `pipeline-watch` / `pipeline-run` YAML config: where it lives, how it
//! is read, and how the zero values are backfilled.

use std::path::PathBuf;

use serde::Deserialize;

use super::DEFAULT_BASE_URL;

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
    let mut config: WatchConfig = serde_norway::from_str(&data)
        .map_err(|error| format!("parse config {display}: {error}"))?;
    config.apply_defaults();
    Ok(config)
}
