// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `build` subcommand: compile the whole module.
//!
//! The Go original installs the dependencies first, builds into `/dev/null`
//! so no binary is left behind, and says nothing when the build succeeds
//! beyond the banner and the closing line.

use std::path::Path;
use std::time::Duration;

use crate::config::Config;
use crate::runner::Runner;

use super::install;

/// The wall-clock budget the build gets.
const BUILD_TIMEOUT: Duration = Duration::from_secs(900);

/// The `build` subcommand: compile the whole module.
pub fn build(cfg: &Config) -> Result<i32, String> {
    println!("🔨 Building project...");
    install::install_dependencies(cfg)
        .map_err(|e| format!("build failed: dependency install error: {e}"))?;
    let runner = Runner::new(cfg.root(), cfg.verbose);
    let args = build_args(cfg);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    if cfg.verbose {
        runner
            .run_interactive("go", &refs)
            .map_err(|e| format!("build failed: {e}"))?;
    } else {
        runner
            .run_silent(BUILD_TIMEOUT, "go", &refs)
            .map_err(|e| format!("build failed: {e}"))?;
    }
    println!("✅ Build successful");
    Ok(0)
}

/// The `go build` command line.
///
/// `-o /dev/null` keeps the build from leaving a binary behind, and
/// `-mod=vendor` is added when the project vendors its dependencies.
fn build_args(cfg: &Config) -> Vec<String> {
    let mut args = vec!["build".to_string()];
    if vendor_exists(cfg.root()) {
        args.push("-mod=vendor".to_string());
    }
    if !cfg.build_tags.is_empty() {
        args.push(format!("-tags={}", cfg.build_tags));
    }
    args.push("-o".to_string());
    args.push("/dev/null".to_string());
    args.push("./...".to_string());
    args
}

/// Whether the project vendors its dependencies.
fn vendor_exists(root: &Path) -> bool {
    std::fs::metadata(root.join("vendor")).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_writes_nothing_to_the_working_tree() {
        let cfg = Config::default();
        let args = build_args(&cfg);
        assert_eq!(args[0], "build");
        assert!(args.contains(&"-o".to_string()));
        assert!(args.contains(&"/dev/null".to_string()));
        assert!(args.contains(&"./...".to_string()));
    }
}
