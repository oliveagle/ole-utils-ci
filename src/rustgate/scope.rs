// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! How much of the workspace a run has to see.
//!
//! [`Mode::Ci`] is the whole workspace. [`Mode::Dev`] is the crates the branch
//! touched — but only under cargo: blaze keeps its own incremental cache and
//! reads a bare `-p` as a *test name filter*, so handing it the crate list
//! would quietly run a subset instead of narrowing the build.
//!
//! [`Mode::Ci`]: crate::config::Mode::Ci
//! [`Mode::Dev`]: crate::config::Mode::Dev

use std::path::Path;

use super::{changed, toolchain};
use crate::config::Config;

/// Lifts a static argument list into the owned form the gates take.
pub fn args_of(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}

/// The arguments a `ci` run hands cargo: the whole workspace.
pub fn whole_workspace(cfg: &Config, leaf: &[&str]) -> Vec<String> {
    let mut args = Vec::new();
    if is_workspace(cfg.root()) {
        args.push("--workspace".to_string());
    }
    args.extend(leaf.iter().map(|arg| arg.to_string()));
    args
}

/// The arguments a `dev` run hands the driver, or `None` when the diff touched
/// no crate and the gate has nothing to do.
pub fn scoped(
    cfg: &Config,
    driver: toolchain::Driver,
    leaf: &[&str],
) -> Result<Option<Vec<String>>, String> {
    if !cfg.is_dev() {
        return Ok(Some(whole_workspace(cfg, leaf)));
    }
    let Some(scope) = changed::changed_crates(cfg)? else {
        return Ok(Some(whole_workspace(cfg, leaf)));
    };
    if scope.is_empty() {
        return Ok(None);
    }
    if driver != toolchain::Driver::Cargo {
        return Ok(Some(args_of(leaf)));
    }
    let mut args = Vec::new();
    for package in &scope.crates {
        args.push("-p".to_string());
        args.push(package.clone());
    }
    args.extend(leaf.iter().map(|arg| arg.to_string()));
    Ok(Some(args))
}

/// Whether the manifest at `root` declares a workspace.
pub fn is_workspace(root: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(root.join("Cargo.toml")) else {
        return false;
    };
    toml::from_str::<toml::Value>(&text)
        .map(|value| value.get("workspace").is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests;
