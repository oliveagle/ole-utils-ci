// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The local git divergence behind the dashboard's MD column.
//!
//! The column answers "how many commits is my local main/master ahead of
//! / behind origin?" by reading a local clone of each monitored repo. It
//! is a local-only signal: no GitLab call, and deliberately no `git fetch`
//! — the comparison runs against the existing remote-tracking ref, exactly
//! like `git status` does. Fetching every repo on every dashboard tick
//! would burn the network for marginal freshness.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::runner::Runner;

use super::rows::RepoRow;

/// The wall-clock budget of one git call.
///
/// A hung repo — a broken alternates file, a stalled filesystem — must
/// never stall the dashboard refresh loop.
const GIT_TIMEOUT: Duration = Duration::from_secs(2);

/// Fills the local-divergence fields of every row.
///
/// A row without a usable local checkout keeps `local_found == false`,
/// which the renderer shows as a dim dash. Safe to call with an empty
/// root. Returns the number of rows that were annotated.
pub fn annotate_local_divergence(rows: &mut [RepoRow], local_root: &str) -> usize {
    if local_root.trim().is_empty() {
        return 0;
    }
    let mut annotated = 0;
    for row in rows.iter_mut() {
        if annotate_row(row, local_root) {
            annotated += 1;
        }
    }
    annotated
}

/// Annotates one row from its local checkout.
fn annotate_row(row: &mut RepoRow, local_root: &str) -> bool {
    let dir = repo_dir(local_root, &row.project);
    let Some(branch) = default_branch(&dir) else {
        return false;
    };
    let Some((ahead, behind)) = divergence(&dir, &branch) else {
        return false;
    };
    row.local_found = true;
    row.local_branch = branch.clone();
    row.local_ahead = ahead;
    row.local_behind = behind;
    // The lead is best-effort: a failed read leaves it at zero.
    row.local_lead = lead(&dir, &branch).unwrap_or(0);
    if !row.latest_tag.is_empty() {
        row.local_tag_behind = tag_behind(&dir, &branch, &row.latest_tag).unwrap_or(0);
    }
    true
}

/// The checkout directory of one project under `root`.
///
/// The last path segment names the checkout, so `devine/devine_utils` lives
/// in `<root>/devine_utils`.
fn repo_dir(root: &str, project_path: &str) -> PathBuf {
    let segment = project_path.rsplit('/').next().unwrap_or(project_path);
    if segment.is_empty() {
        return PathBuf::from(root);
    }
    Path::new(root).join(segment)
}

/// The branch a checkout tracks for the comparison.
///
/// Precedence: the `refs/remotes/origin/HEAD` symref, then a present
/// `origin/main`, then a present `origin/master`. Empty means the directory
/// is not a checkout, or exposes neither.
fn default_branch(dir: &Path) -> Option<String> {
    if let Ok(out) = run_git(
        dir,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) {
        let trimmed = out.trim();
        if !trimmed.is_empty() {
            let name = trimmed.rsplit('/').next().unwrap_or(trimmed);
            return Some(name.to_string());
        }
    }
    if run_git(
        dir,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "refs/remotes/origin/main",
        ],
    )
    .is_ok()
    {
        return Some("main".to_string());
    }
    if run_git(
        dir,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "refs/remotes/origin/master",
        ],
    )
    .is_ok()
    {
        return Some("master".to_string());
    }
    None
}

/// The commits the local branch is ahead of and behind its tracking ref.
///
/// `git rev-list --left-right --count A...B` prints `behind<TAB>ahead`,
/// with the origin side on the left.
fn divergence(dir: &Path, branch: &str) -> Option<(i64, i64)> {
    let spec = format!("refs/remotes/origin/{branch}...refs/heads/{branch}");
    let out = run_git(dir, &["rev-list", "--left-right", "--count", &spec]).ok()?;
    let fields: Vec<&str> = out.split_whitespace().collect();
    if fields.len() != 2 {
        return None;
    }
    let behind = fields[0].parse::<i64>().ok()?;
    let ahead = fields[1].parse::<i64>().ok()?;
    Some((ahead, behind))
}

/// How many commits the checked-out branch leads the remote mainline by.
fn lead(dir: &Path, branch: &str) -> Option<i64> {
    let spec = format!("refs/remotes/origin/{branch}..HEAD");
    let out = run_git(dir, &["rev-list", "--count", &spec]).ok()?;
    let count = out.trim().parse::<i64>().ok()?;
    Some(count.max(0))
}

/// How many commits the newest tag is behind the remote mainline.
fn tag_behind(dir: &Path, branch: &str, tag: &str) -> Option<i64> {
    let spec = format!("refs/tags/{tag}..refs/remotes/origin/{branch}");
    let out = run_git(dir, &["rev-list", "--count", &spec]).ok()?;
    let count = out.trim().parse::<i64>().ok()?;
    Some(count.max(0))
}

/// Runs `git <args...>` in `dir` with a hard timeout.
fn run_git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let runner = Runner::new(dir, false);
    let output = runner.run_with_timeout(GIT_TIMEOUT, "git", args)?;
    if output.success {
        return Ok(output.output);
    }
    let detail = output.output.trim();
    if detail.is_empty() {
        return Err(format!(
            "git {}: exit {}",
            args.join(" "),
            output.code.unwrap_or(-1)
        ));
    }
    Err(format!("git {}: {detail}", args.join(" ")))
}

#[cfg(test)]
mod tests;
