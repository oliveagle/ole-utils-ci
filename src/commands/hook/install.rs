// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `hook install`, `hook uninstall` and `hook status`.
//!
//! The hooks are three-line shell shims that `exec` this binary, so the policy
//! lives in one place and the shims never need to be regenerated when it
//! changes. Each shim names the absolute path it was installed from, so a hook
//! keeps working in a shell where `PATH` does not reach the tool.
//!
//! A hook file that does not carry our marker belongs to somebody else: it is
//! never overwritten without `--force`, and never deleted at all.

use std::path::{Path, PathBuf};

use crate::output::{Diagnostic, Report};

use super::policy::{take_switch, take_value};
use super::{parse_globals, repo_root};

/// The line every shim carries, and the test for "this one is ours".
pub const MARKER: &str =
    "# managed by devine-utils-ci — `devine-utils-ci hook uninstall` removes it";

/// The hooks the command installs.
pub const HOOKS: [&str; 3] = ["pre-commit", "commit-msg", "pre-push"];

/// What a hook file turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// No file at that path.
    Absent,
    /// Our shim.
    Ours,
    /// A hook written by somebody else.
    Foreign,
}

impl State {
    /// Canonical name for `--output json`.
    pub fn as_str(self) -> &'static str {
        match self {
            State::Absent => "absent",
            State::Ours => "ours",
            State::Foreign => "foreign",
        }
    }
}

/// Runs `hook install`.
///
/// Every hook is inspected before anything is written: a repository with one
/// foreign hook is refused as a whole, so a failed install never leaves half a
/// guard behind.
pub fn install(args: &[String]) -> Result<i32, String> {
    let (force, rest) = take_switch(args, "force");
    let (command, rest) = take_value(&rest, "hook-command");
    let (cfg, _positional) = parse_globals(&rest)?;
    let root = repo_root(&cfg)?;
    let dir = hooks_dir(&root)?;
    let program = command.unwrap_or_else(installed_path);
    println!("🔧 hook install: {}", dir.display());
    let planned: Vec<(&str, PathBuf, State)> = HOOKS
        .iter()
        .map(|name| {
            let path = dir.join(name);
            let state = state(&path)?;
            Ok((*name, path, state))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let blocked: Vec<&str> = planned
        .iter()
        .filter(|(_, _, state)| *state == State::Foreign && !force)
        .map(|(name, _, _)| *name)
        .collect();
    if !blocked.is_empty() {
        for (name, path, state) in &planned {
            let label = match state {
                State::Foreign => format!("a different hook is already there — {}", path.display()),
                State::Ours => format!("already ours — {}", path.display()),
                State::Absent => "not installed yet".to_string(),
            };
            let mark = if blocked.contains(name) { "✗" } else { "·" };
            println!("  {mark} {name}: {label}");
        }
        eprintln!(
            "Error: {} hook(s) belong to another tool — rerun with --force to replace them",
            blocked.len()
        );
        return Ok(1);
    }
    let mut report = Report::new("hook install");
    for (name, path, state) in &planned {
        if cfg.dry_run {
            let verb = if *state == State::Ours {
                "would refresh"
            } else {
                "would install"
            };
            println!("  • {name}: {verb} {}", path.display());
            continue;
        }
        write_shim(path, name, &program)?;
        println!("  ✓ {name}: installed {}", path.display());
        report.add(Diagnostic::info(
            path.to_string_lossy(),
            "hook-installed",
            format!("{name} installed"),
        ));
    }
    println!();
    println!("  pre-commit  refuses a commit that is too large, carries a blob or a secret");
    println!("  commit-msg  refuses a message that does not follow <type>: <description>");
    println!("  pre-push    refuses a push to a protected branch or from a stray branch name");
    println!("✅ {} hooks installed in {}", HOOKS.len(), dir.display());
    report.summary = format!("{} hooks installed in {}", HOOKS.len(), dir.display());
    report.extra = Some(serde_json::json!({
        "hooks_dir": dir.to_string_lossy(),
        "command": program,
        "hooks": HOOKS,
    }));
    Ok(0)
}

/// Runs `hook uninstall`.
pub fn uninstall(args: &[String]) -> Result<i32, String> {
    let (cfg, _positional) = parse_globals(args)?;
    let root = repo_root(&cfg)?;
    let dir = hooks_dir(&root)?;
    println!("🧹 hook uninstall: {}", dir.display());
    let mut removed = 0;
    for name in HOOKS {
        let path = dir.join(name);
        match state(&path)? {
            State::Ours if cfg.dry_run => println!("  • {name}: would remove {}", path.display()),
            State::Ours => {
                std::fs::remove_file(&path)
                    .map_err(|e| format!("cannot remove {}: {e}", path.display()))?;
                println!("  ✓ {name}: removed");
                removed += 1;
            }
            State::Foreign => println!("  · {name}: kept (installed by another tool)"),
            State::Absent => println!("  · {name}: not installed"),
        }
    }
    println!("✅ {removed} hooks removed");
    Ok(0)
}

/// Runs `hook status`.
pub fn status(args: &[String]) -> Result<i32, String> {
    let (cfg, _positional) = parse_globals(args)?;
    let root = repo_root(&cfg)?;
    let dir = hooks_dir(&root)?;
    let mut report = Report::new("hook status");
    let mut states = serde_json::Map::new();
    if cfg.structured() {
        // The payload is built first, so a structured caller sees the same
        // facts the text body prints.
        for name in HOOKS {
            let path = dir.join(name);
            let state = state(&path)?;
            states.insert(
                name.to_string(),
                serde_json::Value::String(state.as_str().to_string()),
            );
        }
        report.extra = Some(serde_json::json!({
            "hooks_dir": dir.to_string_lossy(),
            "hooks": states,
        }));
        report.summary = "hook status".to_string();
        print!("{}", report.render(cfg.output_mode()));
        return Ok(0);
    }
    println!("🔎 hook status: {}", dir.display());
    for name in HOOKS {
        let path = dir.join(name);
        let label = match state(&path)? {
            State::Ours => "installed by devine-utils-ci",
            State::Foreign => "installed by another tool",
            State::Absent => "not installed",
        };
        println!("  {name:<12}{label}");
    }
    Ok(0)
}

/// The directory git resolves for hooks, honouring `core.hooksPath`.
fn hooks_dir(root: &Path) -> Result<PathBuf, String> {
    let raw = crate::git::run(root, &["rev-parse", "--git-path", "hooks"])?;
    let path = PathBuf::from(&raw);
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    std::fs::create_dir_all(&path)
        .map_err(|e| format!("cannot create the hooks directory {}: {e}", path.display()))?;
    Ok(path)
}

/// Whether the file at `path` is ours, somebody else's, or missing.
fn state(path: &Path) -> Result<State, String> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.contains(MARKER) => Ok(State::Ours),
        Ok(_) => Ok(State::Foreign),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::Absent),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// Writes the shim that forwards to `hook <name>`.
fn write_shim(path: &Path, name: &str, program: &str) -> Result<(), String> {
    let body = format!("#!/bin/sh\n{MARKER}\nexec '{program}' hook {name} \"$@\"\n");
    std::fs::write(path, body).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    set_executable(path)
}

/// Makes the shim executable (no-op on a platform without Unix modes).
#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(path, mode)
        .map_err(|e| format!("cannot make {} executable: {e}", path.display()))
}

/// Makes the shim executable (no-op on a platform without Unix modes).
#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// The absolute path of the running binary, used to write the shims.
fn installed_path() -> String {
    std::env::current_exe()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| "devine-utils-ci".to_string())
}
