// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Where the topology comes from, and in which order the layers win.
//!
//! Precedence, lowest to highest:
//!
//! 1. the built-in fallback ([`default_workflow_config`]);
//! 2. the user file `~/.devine/utils/workflow.yaml`;
//! 3. the repo-committed file (`.devine/workflow.yaml`, then
//!    `workflow.yaml`, found by walking up from the working directory to the
//!    git root);
//! 4. an explicit `--config PATH`.
//!
//! Any layer may override any workflow by name, including the built-in, and
//! entries merge field by field so a repo file can tweak one value.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::defaults::default_workflow_config;
use super::model::{
    WorkflowConfig, WorkflowFileEntry, DEFAULT_WORKFLOW_CONFIG_PATH, DEFAULT_WORKFLOW_NAME,
};

/// The repo-relative locations searched, in order, for a committed topology.
pub const REPO_WORKFLOW_CONFIG_REL_PATHS: [&str; 2] = [".devine/workflow.yaml", "workflow.yaml"];

/// The working directory and home directory a load reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// The directory the command was started in.
    pub cwd: PathBuf,
    /// `$HOME`, possibly empty (Go's `os.Getenv` returns `""` when unset).
    pub home: String,
}

impl Context {
    /// Reads the process working directory and `$HOME`.
    pub fn current() -> Context {
        Context {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            home: std::env::var("HOME").unwrap_or_default(),
        }
    }
}

/// Every known workflow, keyed by name.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Registry {
    entries: BTreeMap<String, WorkflowConfig>,
}

impl Registry {
    /// The registry holding only the built-in fallback.
    pub fn builtin() -> Registry {
        let mut entries = BTreeMap::new();
        entries.insert(DEFAULT_WORKFLOW_NAME.to_string(), default_workflow_config());
        Registry { entries }
    }

    /// The workflow stored under `name`.
    pub fn get(&self, name: &str) -> Option<&WorkflowConfig> {
        self.entries.get(name)
    }

    /// The built-in workflow (the dispatch fallback).
    pub fn default_workflow(&self) -> Option<&WorkflowConfig> {
        self.get(DEFAULT_WORKFLOW_NAME)
    }

    /// Stores one workflow.
    pub fn insert(&mut self, cfg: WorkflowConfig) {
        self.entries.insert(cfg.name.clone(), cfg);
    }

    /// Every workflow, in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &WorkflowConfig)> {
        self.entries.iter()
    }
}

/// The `workflows:` envelope of a config file.
#[derive(Debug, Default, Deserialize)]
struct WorkflowFile {
    #[serde(default)]
    workflows: BTreeMap<String, Option<WorkflowFileEntry>>,
}

/// Resolves a leading `~` to `$HOME`; an empty path stays empty.
pub fn expand_home(path: &str, home: &str) -> PathBuf {
    match path.strip_prefix('~') {
        Some(rest) => PathBuf::from(format!("{home}{rest}")),
        None => PathBuf::from(path),
    }
}

/// Walks up from `start` looking for a repo-committed workflow config.
///
/// The walk stops at the git root (the first directory holding `.git`) so an
/// unrelated file higher up, for example in `$HOME`, is never mistaken for
/// this repo's topology.
pub fn discover_repo_config(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        for rel in REPO_WORKFLOW_CONFIG_REL_PATHS {
            let candidate = dir.join(rel);
            if matches!(std::fs::metadata(&candidate), Ok(meta) if !meta.is_dir()) {
                return Some(candidate);
            }
        }
        if std::fs::metadata(dir.join(".git")).is_ok() {
            return None;
        }
        match dir.parent() {
            Some(parent) if parent != dir => dir = parent.to_path_buf(),
            _ => return None,
        }
    }
}

/// Layers every entry of the YAML file at `path` onto `reg`.
///
/// A missing or blank file is a no-op. An entry whose name already exists
/// merges onto it (only non-zero fields win); a new name becomes a new
/// workflow. Overriding the built-in name prints one notice to stderr.
pub fn merge_file_into(reg: &mut Registry, path: &Path) -> Result<bool, String> {
    let data = match std::fs::read(path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("read workflow config {}: {error}", path.display())),
    };
    let display = path.display();
    if String::from_utf8_lossy(&data).trim().is_empty() {
        return Ok(true);
    }
    let file: WorkflowFile = serde_norway::from_slice(&data)
        .map_err(|error| format!("parse workflow config {display}: {error}"))?;
    for (name, entry) in file.workflows {
        let Some(entry) = entry else { continue };
        if name.is_empty() {
            return Err(format!(
                "workflow config {display}: empty workflow name under `workflows:`"
            ));
        }
        match reg.get(&name) {
            Some(existing) => {
                if name == DEFAULT_WORKFLOW_NAME {
                    eprintln!(
                        "workflow: {display} overrides the built-in {DEFAULT_WORKFLOW_NAME:?} \
                         (repo-committed config wins over the binary fallback)"
                    );
                }
                let mut merged = existing.clone();
                entry.merge_onto(&mut merged);
                merged.apply_defaults();
                merged.name = name;
                reg.insert(merged);
            }
            None => reg.insert(entry.into_config(&name)),
        }
    }
    Ok(true)
}

/// Loads the full precedence chain for `explicit`, the `--config` value.
pub fn load_layered(explicit: &str, ctx: &Context) -> Result<Registry, String> {
    let mut reg = Registry::builtin();
    let mut paths = vec![expand_home(DEFAULT_WORKFLOW_CONFIG_PATH, &ctx.home)];
    if let Some(repo) = discover_repo_config(&ctx.cwd) {
        paths.push(repo);
    }
    if !explicit.is_empty() {
        paths.push(expand_home(explicit, &ctx.home));
    }
    let mut applied: Vec<PathBuf> = Vec::new();
    for path in paths {
        if path.as_os_str().is_empty() || applied.contains(&path) {
            continue;
        }
        applied.push(path.clone());
        merge_file_into(&mut reg, &path)?;
    }
    Ok(reg)
}

/// Reads `--config` / `-config` out of `args`, in both the space and the
/// `=` forms, so the registry can load before dispatch.
pub fn extract_config_flag(args: &[String]) -> String {
    for (index, arg) in args.iter().enumerate() {
        for prefix in ["--config=", "-config="] {
            if let Some(value) = arg.strip_prefix(prefix) {
                return value.to_string();
            }
        }
        if (arg == "--config" || arg == "-config") && index + 1 < args.len() {
            return args[index + 1].clone();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests;
