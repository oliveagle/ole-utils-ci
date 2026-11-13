// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `hook pre-push`: which refs a push may create or update.
//!
//! Git feeds a pre-push hook one line per ref on standard input:
//!
//! ```text
//! <local ref> <local sha> <remote ref> <remote sha>
//! ```
//!
//! A protected branch is refused whatever the line says, including a delete
//! (`<local sha>` all zeroes). Every other branch must be `<type>/<slug>`, so
//! the remote history stays readable and every push maps to a merge request
//! somebody can open.

use std::io::{IsTerminal, Read};
use std::path::Path;

use crate::output::Diagnostic;

use super::policy::Policy;
use super::{parse_globals, repo_root, Verdict};

/// One ref a push would touch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushRef {
    /// The ref name on the local side, e.g. `refs/heads/fix/x`.
    pub local_ref: String,
    /// The object the local side points at.
    pub local_sha: String,
    /// The ref name on the remote side.
    pub remote_ref: String,
}

impl PushRef {
    /// The branch name a head ref carries.
    pub fn branch(&self) -> Option<&str> {
        self.remote_ref.strip_prefix("refs/heads/")
    }

    /// Whether the push deletes the remote ref.
    pub fn is_delete(&self) -> bool {
        self.local_sha.chars().all(|c| c == '0')
    }
}

/// Runs `hook pre-push [<remote> <url>]`.
pub fn pre_push(args: &[String]) -> Result<i32, String> {
    let (policy, rest) = Policy::parse(args)?;
    let (branch, rest) = super::policy::take_value(&rest, "branch");
    let (cfg, positional) = parse_globals(&rest)?;
    let remote = positional.first().map(String::as_str).unwrap_or("origin");
    let root = repo_root(&cfg)?;
    let refs = collect_refs(&root, branch.as_deref())?;
    let mut verdict = Verdict::new("hook pre-push");
    let header = format!(
        "🚀 pre-push → {remote}: {} {}",
        refs.len(),
        super::plural(refs.len(), "ref")
    );
    if refs.is_empty() {
        verdict.pass("nothing to push");
        return Ok(verdict.finish(&cfg, &header, "pre-push passed"));
    }
    for reference in &refs {
        judge(&policy, reference, &mut verdict);
    }
    let extra = serde_json::json!({
        "remote": remote,
        "protected_branches": policy.protected,
        "refs": refs.iter().map(|r| r.remote_ref.clone()).collect::<Vec<_>>(),
    });
    Ok(verdict
        .with_extra(extra)
        .finish(&cfg, &header, "pre-push passed"))
}

/// Applies the branch policy to one ref.
fn judge(policy: &Policy, reference: &PushRef, verdict: &mut Verdict) {
    if reference.remote_ref.starts_with("refs/tags/") {
        let name = reference.remote_ref.trim_start_matches("refs/tags/");
        verdict.pass(format!("tag {name} may be pushed"));
        return;
    }
    let Some(branch) = reference.branch() else {
        verdict.pass(format!(
            "{} is not a branch — allowed",
            reference.remote_ref
        ));
        return;
    };
    if policy.is_protected(branch) {
        let message = format!(
            "{branch} is protected — push your work to a <type>/<slug> branch and open a merge request"
        );
        verdict.deny(Diagnostic::error("", "protected-branch", &message), message);
        return;
    }
    match policy.branch_problem(branch) {
        None => {
            let verb = if reference.is_delete() {
                "may be deleted"
            } else {
                "may be pushed"
            };
            verdict.pass(format!("branch {branch} {verb}"));
        }
        Some(problem) => {
            verdict.deny(Diagnostic::error("", "branch-name", &problem), problem);
        }
    }
}

/// The refs this push would touch.
///
/// Git always pipes them in. A person running the hook by hand has a terminal
/// instead, so the checked-out branch (or `--branch`) stands in for them.
fn collect_refs(root: &Path, branch: Option<&str>) -> Result<Vec<PushRef>, String> {
    if let Some(branch) = branch {
        return Ok(vec![PushRef {
            local_ref: format!("refs/heads/{branch}"),
            local_sha: String::new(),
            remote_ref: format!("refs/heads/{branch}"),
        }]);
    }
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        let current = crate::git::current_branch(root)?;
        return Ok(vec![PushRef {
            local_ref: format!("refs/heads/{current}"),
            local_sha: String::new(),
            remote_ref: format!("refs/heads/{current}"),
        }]);
    }
    let mut buffer = String::new();
    stdin
        .lock()
        .read_to_string(&mut buffer)
        .map_err(|e| format!("cannot read the refs git passed to pre-push: {e}"))?;
    Ok(parse_refs(&buffer))
}

/// Parses the `pre-push` standard input.
pub fn parse_refs(text: &str) -> Vec<PushRef> {
    text.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 4 {
                return None;
            }
            Some(PushRef {
                local_ref: fields[0].to_string(),
                local_sha: fields[1].to_string(),
                remote_ref: fields[2].to_string(),
            })
        })
        .collect()
}
