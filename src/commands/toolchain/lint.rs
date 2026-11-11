// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `lint` subcommand: the composite static gate.
//!
//! The order is the Go original's: the dependencies are installed first, the
//! project configuration is read, then the Go tools run, then the
//! config-driven gates — the toolchain pin, the shell scan, the plugin
//! freshness and the web syntax pass. Every gate renders its findings the
//! way the Go text renderer does and stops the run at the first failure.

use std::path::Path;
use std::time::Duration;

use crate::config::Config;
use crate::gates::{focused, gopin, noshell, plugin_freshness};
use crate::lint_config::{self, Resolved};
use crate::output::{Diagnostic, Mode};
use crate::runner::Runner;

use super::install;

/// The wall-clock budget one Go tool gets.
const TOOL_TIMEOUT: Duration = Duration::from_secs(300);

/// The wall-clock budget one silent Go tool gets.
///
/// The Go original gives `go vet` and `golangci-lint` no deadline at all;
/// the budget here keeps a wedged tool from stalling the pipeline.
const SILENT_TIMEOUT: Duration = Duration::from_secs(900);

/// The `lint` subcommand: the composite static gate.
pub fn lint(cfg: &Config) -> Result<i32, String> {
    install::install_dependencies(cfg).map_err(|e| format!("lint: install: {e}"))?;
    let config =
        lint_config::load(cfg.root()).map_err(|e| format!("lint: load project config: {e}"))?;
    let resolved = lint_config::resolve(cfg.root(), &config);
    let runner = Runner::new(cfg.root(), cfg.verbose);
    if resolved.go_format {
        gofmt(&runner, cfg.root(), &resolved.first_party_dirs)?;
    }
    if resolved.go_mod_format {
        go_mod_format(&runner)?;
    }
    if resolved.go_vet {
        go_vet(&runner, cfg)?;
    }
    if resolved.golangci_lint && Runner::command_exists("golangci-lint") {
        golangci_lint(&runner, cfg)?;
    }
    go_pin_gate(cfg, &resolved)?;
    no_shell_gate(cfg, &resolved)?;
    plugin_gate(cfg, &resolved)?;
    web_gate(cfg, &resolved)?;
    Ok(0)
}

/// Runs `gofmt -s` over the first-party trees, fixing what it can.
fn gofmt(runner: &Runner, root: &Path, dirs: &[String]) -> Result<(), String> {
    let mut targets = existing_dirs(root, dirs);
    if targets.is_empty() {
        targets.push(".".to_string());
    }
    let listing = runner.run_go(TOOL_TIMEOUT, "gofmt", &gofmt_args(&["-l"], &targets));
    if let Some(error) = listing.error {
        return Err(format!("gofmt: {error}"));
    }
    if listing.output.split_whitespace().next().is_none() {
        return Ok(());
    }
    let fix = runner.run_go(TOOL_TIMEOUT, "gofmt", &gofmt_args(&["-s", "-w"], &targets));
    if let Some(error) = fix.error {
        return Err(format!("gofmt -s -w: {error}"));
    }
    let still = runner.run_go(TOOL_TIMEOUT, "gofmt", &gofmt_args(&["-l"], &targets));
    let remaining = still.output;
    if !remaining.trim().is_empty() {
        let count = remaining.split_whitespace().count();
        return Err(format!(
            "gofmt: {count} file(s) still unformatted after fix:\n{remaining}"
        ));
    }
    Ok(())
}

/// The command line for one `gofmt` call.
fn gofmt_args<'a>(flags: &[&'a str], dirs: &'a [String]) -> Vec<&'a str> {
    let mut args: Vec<&str> = flags.to_vec();
    args.extend(dirs.iter().map(String::as_str));
    args
}

/// The first-party directories that exist under `root`.
fn existing_dirs(root: &Path, rels: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for rel in rels {
        let cleaned = clean(rel);
        if cleaned == "." {
            continue;
        }
        if root.join(&cleaned).is_dir() {
            out.push(cleaned);
        }
    }
    out
}

/// `filepath.Clean`: the path with `.` and `..` resolved.
fn clean(rel: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in rel.split('/') {
        match part {
            "" | "." => continue,
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// Rewrites `go.mod` in the canonical form.
fn go_mod_format(runner: &Runner) -> Result<(), String> {
    let result = runner.run_go(TOOL_TIMEOUT, "go", &["mod", "edit", "-fmt"]);
    match result.error {
        Some(error) => Err(format!("go mod edit -fmt: {error}")),
        None => Ok(()),
    }
}

/// Runs `go vet` over the module.
fn go_vet(runner: &Runner, cfg: &Config) -> Result<(), String> {
    let args = vet_args(cfg);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    if cfg.verbose {
        return runner
            .run_interactive("go", &refs)
            .map_err(|e| format!("go vet: {e}"));
    }
    runner
        .run_silent(SILENT_TIMEOUT, "go", &refs)
        .map_err(|e| format!("go vet: {e}"))
}

/// The `go vet` command line.
fn vet_args(cfg: &Config) -> Vec<String> {
    let mut args = vec!["vet".to_string()];
    if !cfg.build_tags.is_empty() {
        args.push(format!("-tags={}", cfg.build_tags));
    }
    args.push("./...".to_string());
    args
}

/// Runs `golangci-lint` when the project has it installed.
fn golangci_lint(runner: &Runner, cfg: &Config) -> Result<(), String> {
    let args = ["run", "./..."];
    if cfg.verbose {
        return runner
            .run_interactive("golangci-lint", &args)
            .map_err(|e| format!("golangci-lint: {e}"));
    }
    runner
        .run_silent(SILENT_TIMEOUT, "golangci-lint", &args)
        .map_err(|e| format!("golangci-lint: {e}"))
}

/// The `go-pin` gate, when the project configuration enables it.
fn go_pin_gate(cfg: &Config, resolved: &Resolved) -> Result<(), String> {
    if !resolved.go_pin.enabled {
        return Ok(());
    }
    println!("==> go-pin");
    let settings = gopin::Settings {
        version_file: resolved.go_pin.version_file.clone(),
        ci_file: resolved.go_pin.ci_file.clone(),
        dockerfile: resolved.go_pin.dockerfile.clone(),
        require_go_script: resolved.go_pin.require_go_script.clone(),
        with_toolchain: resolved.go_pin.with_toolchain,
    };
    let (findings, extra) =
        gopin::check(cfg.root(), &settings).map_err(|e| format!("go-pin: {e}"))?;
    focused::render("go-pin", &findings, extra, Mode::Text).map_err(|e| format!("go-pin: {e}"))
}

/// The `no-shell-exec` gate, when the project configuration enables it.
fn no_shell_gate(cfg: &Config, resolved: &Resolved) -> Result<(), String> {
    if !resolved.no_shell.enabled {
        return Ok(());
    }
    println!("==> no-shell-exec");
    let (findings, scanned) = noshell::scan(
        cfg.root(),
        &resolved.no_shell.dirs,
        &resolved.no_shell.forbidden_paths,
    );
    let diagnostics: Vec<Diagnostic> = findings.iter().map(noshell::to_diagnostic).collect();
    let extra = serde_json::json!({
        "dirs": resolved.no_shell.dirs,
        "files_scanned": scanned,
    });
    focused::render("no-shell-exec", &diagnostics, Some(extra), Mode::Text)
        .map_err(|e| format!("no-shell-exec: {e}"))
}

/// The `plugin-freshness` gate, when the project configuration enables it.
fn plugin_gate(cfg: &Config, resolved: &Resolved) -> Result<(), String> {
    if !resolved.plugin.enabled {
        return Ok(());
    }
    println!("==> plugin-freshness");
    let (findings, extra) = plugin_freshness::check(
        cfg.root(),
        &resolved.plugin.source_dir,
        &resolved.plugin.plugins_dir,
    )
    .map_err(|e| format!("plugin-freshness: {e}"))?;
    focused::render("plugin-freshness", &findings, Some(extra), Mode::Text)
        .map_err(|e| format!("plugin-freshness: {e}"))
}

/// The `web-lint` gate, when the project configuration enables it.
fn web_gate(cfg: &Config, resolved: &Resolved) -> Result<(), String> {
    if !resolved.web.enabled {
        return Ok(());
    }
    println!("==> web-lint");
    for path in &resolved.web.paths {
        let options = crate::commands::web::lint::Options {
            project_root: cfg.root().to_string_lossy().into_owned(),
            path: path.clone(),
            content: String::new(),
            language: String::new(),
            root: cfg.root().to_string_lossy().into_owned(),
            ts_engine: resolved.web.ts_engine.clone(),
            tsconfig: resolved.web.tsconfig.clone(),
            ignore: resolved.web.ignore.clone(),
            max_files: web_limit(resolved.web.max_files, 200),
            max_file_size: web_limit(resolved.web.max_file_size, 1_048_576),
            format: "text".to_string(),
            fail_on: resolved.web.fail_on.clone(),
        };
        crate::commands::web::lint::lint_paths(&options).map_err(|e| format!("web-lint: {e}"))?;
    }
    Ok(())
}

/// The cap the engine uses when the project names none.
fn web_limit(configured: i64, fallback: i64) -> i64 {
    if configured > 0 {
        configured
    } else {
        fallback
    }
}
