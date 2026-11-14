// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `git-author` command: per-repository commit-identity policy.
//!
//! Every commit against a company GitLab (`git.dev.sh.ctripcorp.com`, …) must
//! be authored as `<username>@trip.com`. Contributors routinely mix
//! personal-machine defaults with corporate remotes, which produces history
//! the corporate server later rejects — or silently accepts under the wrong
//! identity. This command surfaces and fixes that mismatch.
//!
//! The company rule is baked in; `~/.devine/utils/ci/git-author.yaml` may
//! prepend rules of its own. Rules are evaluated in order and the first match
//! wins, so user rules take precedence while the corporate rule stays the
//! safety net.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// One URL→identity policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitAuthorRule {
    /// Matches when the remote URL contains this string (case-insensitive).
    /// Empty matches every remote.
    #[serde(default)]
    pub remote_substring: String,
    /// The mandatory email suffix, including the leading `@`.
    #[serde(default)]
    pub require_suffix: String,
    /// Template for the expected `user.name`. `{local}` is the email
    /// local-part; empty means the name is not policed.
    #[serde(default)]
    pub name_format: String,
    /// Why this rule fired, surfaced in the report.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// The on-disk YAML.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitAuthorConfig {
    /// The effective rule list, user rules first.
    #[serde(default)]
    pub rules: Vec<GitAuthorRule>,
}

/// The hard-coded company policy.
fn default_rules() -> Vec<GitAuthorRule> {
    vec![GitAuthorRule {
        remote_substring: "ctripcorp.com".to_string(),
        require_suffix: "@trip.com".to_string(),
        name_format: "{local}".to_string(),
        description: "Corporate GitLab: commits must use <username>@trip.com".to_string(),
    }]
}

/// `~/.devine/utils/ci/git-author.yaml`, or an empty string when there is no
/// home directory.
pub fn default_config_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        return String::new();
    }
    PathBuf::from(home)
        .join(".devine")
        .join("utils")
        .join("ci")
        .join("git-author.yaml")
        .to_string_lossy()
        .to_string()
}

/// Loads the user rules and prepends them to the defaults.
///
/// A missing file is a soft miss; a malformed file is a hard error.
pub fn load_config(path: &str) -> Result<GitAuthorConfig, String> {
    let mut config = GitAuthorConfig {
        rules: default_rules(),
    };
    let mut path = path.to_string();
    if path.is_empty() {
        path = default_config_path();
    }
    if path.is_empty() {
        return Ok(config);
    }
    let data = match std::fs::read_to_string(&path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(config),
        Err(e) => return Err(format!("read {path}: {e}")),
    };
    let user: GitAuthorConfig =
        serde_norway::from_str(&data).map_err(|e| format!("parse {path}: {e}"))?;
    let mut rules = user.rules;
    rules.append(&mut config.rules);
    config.rules = rules;
    Ok(config)
}

/// The current repository identity.
#[derive(Debug, Clone, Default)]
pub struct Identity {
    /// `user.name`.
    pub name: String,
    /// `user.email`.
    pub email: String,
    /// The `origin` remote URL.
    pub remote: String,
    /// Whether `user.email` is set at repository scope.
    pub local: bool,
}

/// Reads the current identity and the primary remote URL.
pub fn gather(root: &std::path::Path) -> Identity {
    let mut id = Identity::default();
    if let Ok(email) = crate::git::run(root, &["config", "--get", "user.email"]) {
        id.email = email;
    }
    if let Ok(name) = crate::git::run(root, &["config", "--get", "user.name"]) {
        id.name = name;
    }
    if let Ok(local) = crate::git::run(root, &["config", "--local", "--get", "user.email"]) {
        if !local.trim().is_empty() {
            id.local = true;
        }
    }
    if let Ok(remote) = crate::git::run(root, &["remote", "get-url", "origin"]) {
        id.remote = remote;
    }
    id
}

mod policy;

pub use policy::{check, expected_identity, match_rule, Verdict};

#[cfg(test)]
mod tests {
    use super::*;

    fn corporate() -> GitAuthorRule {
        default_rules().remove(0)
    }

    #[test]
    fn user_rules_take_precedence_over_the_defaults() {
        let mut config = GitAuthorConfig::default();
        let user = GitAuthorRule {
            remote_substring: "example.com".to_string(),
            require_suffix: "@example.com".to_string(),
            name_format: String::new(),
            description: "example".to_string(),
        };
        config.rules.push(user);
        config.rules.append(&mut default_rules());
        assert_eq!(config.rules.len(), 2);
        let matched = match_rule(&config.rules, "https://git.example.com/x/y.git").unwrap();
        assert_eq!(matched.require_suffix, "@example.com");
    }

    #[test]
    fn the_corporate_rule_matches_ctripcorp_remotes() {
        let rule = corporate();
        let rules = [rule];
        let matched = match_rule(&rules, "git@git.dev.sh.ctripcorp.com:devine/x.git").unwrap();
        assert_eq!(matched.require_suffix, "@trip.com");
        assert!(match_rule(&[corporate()], "git@github.com:oliveagle/ole-utils-ci.git").is_none());
    }

    #[test]
    fn the_expected_identity_keeps_the_local_part() {
        let rule = corporate();
        let id = Identity {
            name: "Olive Eagle".to_string(),
            email: "oliveagle@gmail.com".to_string(),
            ..Identity::default()
        };
        let (email, name) = expected_identity(&rule, &id);
        assert_eq!(email, "oliveagle@trip.com");
        assert_eq!(name, "oliveagle");
    }

    #[test]
    fn a_violation_is_reported_with_a_reason() {
        let config = GitAuthorConfig {
            rules: default_rules(),
        };
        let id = Identity {
            name: "oliveagle".to_string(),
            email: "oliveagle@gmail.com".to_string(),
            remote: "https://git.dev.sh.ctripcorp.com/devine/x.git".to_string(),
            ..Identity::default()
        };
        let verdict = check(&config, &id);
        assert!(!verdict.ok);
        assert!(verdict.reason.contains("@trip.com"));
    }

    #[test]
    fn a_compliant_identity_passes() {
        let config = GitAuthorConfig {
            rules: default_rules(),
        };
        let id = Identity {
            name: "oliveagle".to_string(),
            email: "oliveagle@trip.com".to_string(),
            remote: "https://git.dev.sh.ctripcorp.com/devine/x.git".to_string(),
            ..Identity::default()
        };
        assert!(check(&config, &id).ok);
    }
}
