// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Shared helpers for the workflow commands: the git operations on the
//! binary repo, the version-file accessors and the small formatting helpers.
//!
//! Every helper mirrors one Go function in `internal/workflow_util.go`; the
//! wordings are the ones the workflow commands surface, so they live here
//! rather than in the generic `git` module. The two HTTP polls are in
//! `http.rs`.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::runner::{CmdOutput, Runner};
use crate::version::Version;

use super::model::{WorkflowConfig, DEFAULT_WORKFLOW_CONFIG_PATH};

/// Runs git in `runner`'s directory, wording a failure the way Go's
/// `Runner.Run` does (`git <args> failed: exit status 1` plus the output).
pub fn run_git(runner: &Runner, args: &[&str]) -> Result<String, String> {
    let timeout = crate::runner::DEFAULT_CMD_TIMEOUT;
    let output: CmdOutput = runner.run_with_timeout(timeout, "git", args)?;
    if output.success {
        return Ok(output.output);
    }
    let code = match output.code {
        Some(code) => format!("exit status {code}"),
        None => "signal: killed".to_string(),
    };
    Err(format!(
        "git {} failed: {code}\nOutput:\n{}",
        args.join(" "),
        output.output
    ))
}

/// The checked-out branch, honouring `CI_COMMIT_REF_NAME` the way the Go
/// original's `getCurrentBranch` does.
pub fn current_branch(runner: &Runner) -> Result<String, String> {
    let branch = run_git(runner, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let branch = branch.trim().to_string();
    match std::env::var("CI_COMMIT_REF_NAME") {
        Ok(ci) if !ci.is_empty() => Ok(ci),
        _ => Ok(branch),
    }
}

/// The bare version stored in `<dir>/<rel>`, trimmed.
pub fn read_version_from_file(dir: &Path, rel: &str) -> Result<String, String> {
    let data = std::fs::read_to_string(dir.join(rel)).map_err(|error| error.to_string())?;
    Ok(data.trim().to_string())
}

/// Writes the bare version, always with a trailing newline.
pub fn write_version_to_file(dir: &Path, rel: &str, version: &str) -> Result<(), String> {
    std::fs::write(dir.join(rel), format!("{version}\n")).map_err(|error| error.to_string())
}

/// Whether `dir` is a checkout of the workflow's binary repo: it must hold the
/// version file and its git remote must name the expected project.
pub fn is_binary_repo_dir(dir: &Path, cfg: &WorkflowConfig) -> bool {
    let version_file = if cfg.binary_repo.version_file.is_empty() {
        "VERSION"
    } else {
        cfg.binary_repo.version_file.as_str()
    };
    if std::fs::metadata(dir.join(version_file)).is_err() {
        return false;
    }
    let runner = Runner::new(dir, false);
    let remote = format!("remote.{}.url", cfg.binary_repo.remote);
    let url = match run_git(&runner, &["config", "--get", &remote]) {
        Ok(url) => url.trim().to_string(),
        Err(_) => return false,
    };
    if url.is_empty() {
        return false;
    }
    url.contains(&cfg.binary_repo.name) || url.contains(&cfg.binary_repo.project_path)
}

/// Locates the binary repo checkout, or explains why it could not.
pub fn ensure_binary_repo_dir(cfg: &WorkflowConfig, cwd: &Path) -> Result<PathBuf, String> {
    if is_binary_repo_dir(cwd, cfg) {
        return Ok(cwd.to_path_buf());
    }
    Err(format!(
        "workflow: cannot locate the binary repo ({}). Run this command from inside {}, \
         or add a local-path override to {} (not yet wired in v1).",
        cfg.binary_repo.name, cfg.binary_repo.project_path, DEFAULT_WORKFLOW_CONFIG_PATH
    ))
}

/// Fails when the tree has tracked changes; untracked files are allowed
/// (mirror and version files may be added later).
pub fn git_repo_clean(runner: &Runner) -> Result<(), String> {
    let out = run_git(runner, &["status", "--porcelain", "--untracked-files=no"])?;
    if out.trim().is_empty() {
        return Ok(());
    }
    Err(format!("working tree has tracked changes:\n{out}"))
}

/// Checks the branch is main/master, then fetches and fast-forwards it.
///
/// The fetch, the pull and the "no remote ref" case are non-fatal: the
/// release safety check at the end is what actually decides.
pub fn git_ensure_on_main(
    runner: &Runner,
    remote: &str,
    auto_checkout: bool,
) -> Result<(), String> {
    let branch = current_branch(runner)?;
    if branch != "main" && branch != "master" {
        if !auto_checkout {
            return Err(format!(
                "on branch {branch:?} — pass --no-auto-checkout only when already on main; run from main"
            ));
        }
        if let Err(error) = run_git(runner, &["checkout", "main"]) {
            return Err(format!("git checkout main failed: {error}"));
        }
    }
    let remote = if remote.is_empty() { "origin" } else { remote };
    let _ = run_git(runner, &["fetch", remote, "--prune"]);
    let _ = run_git(runner, &["pull", "--ff-only", remote, "main"]);
    let head = run_git(runner, &["rev-parse", "HEAD"])?;
    let tracking = format!("{remote}/main");
    let origin = match run_git(runner, &["rev-parse", &tracking]) {
        Ok(origin) => origin,
        // No remote ref yet: let the push decide.
        Err(_) => return Ok(()),
    };
    let head = head.trim();
    let origin = origin.trim();
    if head != origin {
        return Err(format!(
            "local main HEAD ({head}) != {remote}/main ({origin}) — pull first or push will be rejected"
        ));
    }
    Ok(())
}

/// The URL of one artifact: `<base>/<path>/<version>/<binary-name>`.
pub fn artifactory_url(cfg: &WorkflowConfig, path: &str, version: &str) -> String {
    let base = cfg.artifactory.base_url.trim_end_matches('/');
    let rel = path.trim_matches('/');
    format!("{base}/{rel}/{version}/{}", cfg.binary_repo.name)
}

/// The next version for a bump type, plus the transition's short label.
///
/// Only the two shapes the workflow uses exist: `rc` (stable becomes the next
/// patch's `rc.1`, an rc increments) and `promote` (an rc becomes its stable).
/// The wordings match the Go original, `%q` quoting included.
pub fn next_version(current: &str, bump_type: &str) -> Result<(String, String), String> {
    let version = Version::parse(current)?;
    match bump_type {
        "rc" => {
            let next = if version.is_rc {
                format!(
                    "{}.{}.{}-rc.{}",
                    version.major,
                    version.minor,
                    version.patch,
                    version.rc + 1
                )
            } else {
                format!(
                    "{}.{}.{}-rc.1",
                    version.major,
                    version.minor,
                    version.patch + 1
                )
            };
            Ok((next, "rc".to_string()))
        }
        "promote" => {
            if !version.is_rc {
                return Err(format!(
                    "current version {current:?} is already stable — nothing to promote"
                ));
            }
            Ok((
                format!("{}.{}.{}", version.major, version.minor, version.patch),
                "promote".to_string(),
            ))
        }
        other => Err(format!(
            "unknown bump type {other:?} — use patch, minor, major, rc, or promote"
        )),
    }
}

/// `a` when it carries a value, else `b`.
pub fn non_empty_or(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.to_string()
    } else {
        a.to_string()
    }
}

/// Prints `value` as indented JSON plus the newline `fmt.Println` adds.
pub fn write_output_json<T: Serialize>(value: &T) -> Result<(), String> {
    let text =
        serde_json::to_string_pretty(value).map_err(|error| format!("encode json: {error}"))?;
    println!("{text}");
    Ok(())
}

/// Go's `%v` rendering of a `[]string`: `[a b c]`, without quotes.
pub fn go_slice(items: &[String]) -> String {
    format!("[{}]", items.join(" "))
}

/// The first `limit` characters of `text`, for a truncated error body.
pub fn first_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

#[cfg(test)]
mod tests;
