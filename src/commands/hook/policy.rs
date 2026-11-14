// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The policy the git hooks enforce.
//!
//! Every budget has a default that is good for a normal repository, and every
//! default is overridable: the flag wins over the environment variable, which
//! wins over the built-in value. That order lets a CI job tighten the policy
//! for one call without editing a file, and lets a developer loosen it locally
//! without touching the repository.
//!
//! Why these defaults:
//!
//! * **20 files per commit.** Past that point a single `git show --stat` no
//!   longer reads as one idea, and review becomes "did anything look broken"
//!   instead of "is this correct". Real atomic commits — including a rename
//!   that touches a module boundary — stay well under it.
//! * **5 MiB per staged file.** Git never forgets a blob, so a large binary
//!   committed once costs every clone from then on. Source files are orders of
//!   magnitude smaller; images and fixtures belong in the 5–50 MiB band only
//!   with a deliberate decision.
//! * **`<type>/<slug>` branches.** A branch whose name starts with the kind of
//!   change it carries is what makes `git branch` readable, ties the branch to
//!   the commit convention, and gives the protected-branch rule something to
//!   point at: `main` is not a place for work, so it must not be a branch you
//!   push to.

use super::args::env_value;
pub use super::args::{human_bytes, parse_size, take_switch, take_value};

/// Default cap on the number of files one commit may touch.
pub const MAX_FILES_DEFAULT: usize = 20;

/// Default cap on the size of one staged file, in bytes (5 MiB).
pub const MAX_FILE_BYTES_DEFAULT: u64 = 5 * 1024 * 1024;

/// Longest accepted first line of a commit message.
pub const SUBJECT_MAX: usize = 100;

/// Branch type prefixes that may be pushed.
///
/// `hotfix` and `release` are allowed on top of the commit types because those
/// branches exist to carry a release, not a feature.
pub const BRANCH_TYPES: [&str; 13] = [
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert",
    "hotfix", "release",
];

/// Commit message types accepted by `commit-msg`.
pub const COMMIT_TYPES: [&str; 11] = [
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert",
];

/// Branch names that are never pushed to directly.
pub const PROTECTED_DEFAULT: [&str; 2] = ["main", "master"];

/// Paths that must never be committed, matched against the file name.
pub const SECRET_PATHS: [&str; 7] = [
    ".env",
    ".env.local",
    "id_rsa",
    "id_ed25519",
    "credentials.json",
    "secrets.yaml",
    "secrets.yml",
];

/// Path suffixes that must never be committed.
pub const SECRET_SUFFIXES: [&str; 3] = [".pem", ".key", ".p12"];

/// The limits one hook run applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    /// Maximum number of files a commit may touch.
    pub max_files: usize,
    /// Maximum size of one staged file, in bytes.
    pub max_file_bytes: u64,
    /// Branch names that only accept merges (never a direct push).
    pub protected: Vec<String>,
    /// Reject `.env`, `*.pem` and friends instead of only reporting them.
    pub block_secrets: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            max_files: MAX_FILES_DEFAULT,
            max_file_bytes: MAX_FILE_BYTES_DEFAULT,
            protected: PROTECTED_DEFAULT.iter().map(|s| (*s).to_string()).collect(),
            block_secrets: true,
        }
    }
}

impl Policy {
    /// Whether `branch` only accepts merges.
    pub fn is_protected(&self, branch: &str) -> bool {
        self.protected.iter().any(|name| name == branch)
    }

    /// Why `branch` may not be pushed, or `None` when it may.
    pub fn branch_problem(&self, branch: &str) -> Option<String> {
        let (kind, slug) = match branch.split_once('/') {
            Some(parts) => parts,
            None => {
                return Some(format!(
                    "{branch:?} is not a <type>/<slug> branch — use one of {} (e.g. fix/ci-hook)",
                    BRANCH_TYPES.join(", ")
                ))
            }
        };
        if !BRANCH_TYPES.contains(&kind) {
            return Some(format!(
                "branch {branch:?} has unknown type {kind:?} — use one of {}",
                BRANCH_TYPES.join(", ")
            ));
        }
        if slug.is_empty() || !slug.chars().all(is_slug_char) {
            return Some(format!(
                "branch {branch:?} needs a slug of a-z, 0-9, '.', '_' or '-'"
            ));
        }
        None
    }

    /// Why the first line of `message` breaks the commit convention, if it does.
    pub fn message_problem(&self, message: &str) -> Option<String> {
        let Some(subject) = first_line(message) else {
            return Some("the commit message is empty".to_string());
        };
        if subject.starts_with("Merge ") || subject.starts_with("Revert \"") {
            return None;
        }
        let subject = subject
            .strip_prefix("fixup! ")
            .or_else(|| subject.strip_prefix("squash! "))
            .unwrap_or(subject);
        let (head, description) = match subject.split_once(": ") {
            Some(parts) => parts,
            None => {
                return Some(format!(
                    "{subject:?} is not `<type>: <description>` — use one of {}",
                    COMMIT_TYPES.join(", ")
                ))
            }
        };
        let head = head.trim_end_matches('!');
        let kind = head.split('(').next().unwrap_or(head);
        if !COMMIT_TYPES.contains(&kind) {
            return Some(format!(
                "commit type {kind:?} is not one of {}",
                COMMIT_TYPES.join(", ")
            ));
        }
        if head.contains('(') && !head.ends_with(')') {
            return Some(format!("scope in {head:?} is not closed"));
        }
        if description.trim().is_empty() {
            return Some("the commit description is empty".to_string());
        }
        if subject.chars().count() > SUBJECT_MAX {
            return Some(format!(
                "the subject is {} characters, over the {SUBJECT_MAX}-character budget",
                subject.chars().count()
            ));
        }
        None
    }

    /// Reads the policy from the environment, then lets flags override it.
    ///
    /// Returns the policy and the arguments the shared parser still owns.
    pub fn parse(args: &[String]) -> Result<(Policy, Vec<String>), String> {
        let mut policy = Policy::from_env()?;
        let (raw, rest) = take_value(args, "max-files");
        if let Some(raw) = raw {
            policy.max_files = raw
                .parse()
                .map_err(|_| format!("--max-files expects a number, got {raw:?}"))?;
        }
        let (raw, rest) = take_value(&rest, "max-file-bytes");
        if let Some(raw) = raw {
            policy.max_file_bytes = parse_size(&raw)?;
        }
        let (raw, rest) = take_value(&rest, "protected");
        if let Some(raw) = raw {
            policy.protected = raw
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect();
        }
        let (allow, rest) = take_switch(&rest, "allow-secret-paths");
        if allow {
            policy.block_secrets = false;
        }
        Ok((policy, rest))
    }

    /// The policy the environment selects, before any flag is applied.
    fn from_env() -> Result<Policy, String> {
        let mut policy = Policy::default();
        if let Some(raw) = env_value("HOOK_MAX_FILES") {
            policy.max_files = raw
                .parse()
                .map_err(|_| format!("HOOK_MAX_FILES expects a number, got {raw:?}"))?;
        }
        if let Some(raw) = env_value("HOOK_MAX_FILE_BYTES") {
            policy.max_file_bytes = parse_size(&raw)?;
        }
        if let Some(raw) = env_value("HOOK_PROTECTED_BRANCHES") {
            policy.protected = raw
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect();
        }
        Ok(policy)
    }
}

/// Whether `c` may appear in a branch slug.
fn is_slug_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')
}

/// The first line of a message that is neither blank nor a git comment.
fn first_line(message: &str) -> Option<&str> {
    message
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
}
