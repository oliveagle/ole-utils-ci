// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The policy itself: which rule applies, what identity it wants, and whether
//! the repository already satisfies it.

use super::{GitAuthorConfig, GitAuthorRule, Identity};

/// The first rule whose substring matches the remote; empty matches anything.
pub fn match_rule<'a>(rules: &'a [GitAuthorRule], remote: &str) -> Option<&'a GitAuthorRule> {
    let lower = remote.to_lowercase();
    rules.iter().find(|r| {
        r.remote_substring.is_empty() || lower.contains(&r.remote_substring.to_lowercase())
    })
}

/// The local-part of an email address, or the whole (trimmed) string.
fn local_part(value: &str) -> String {
    match value.find('@') {
        Some(index) if index > 0 => value[..index].to_string(),
        _ => value.trim().to_string(),
    }
}

/// Applies a matched rule to the current identity.
///
/// The local-part is taken from the current identity so users are told to
/// change their *suffix*, not their whole identity.
pub fn expected_identity(rule: &GitAuthorRule, current: &Identity) -> (String, String) {
    let mut local = local_part(&current.email);
    if local.is_empty() {
        local = local_part(&current.name);
    }
    let email = format!("{}{}", local, rule.require_suffix);
    let name = if rule.name_format.is_empty() {
        current.name.clone()
    } else {
        rule.name_format.replace("{local}", &local)
    };
    (email, name)
}

/// The verdict of evaluating the identity against the rule list.
pub struct Verdict {
    /// The rule that matched, when there is one.
    pub rule: Option<GitAuthorRule>,
    /// The email the repository should use.
    pub expected_email: String,
    /// The name the repository should use.
    pub expected_name: String,
    /// Whether the current identity already satisfies the rule.
    pub ok: bool,
    /// Why the identity does or does not satisfy it.
    pub reason: String,
}

/// Evaluates the current identity against the matching rule.
pub fn check(config: &GitAuthorConfig, id: &Identity) -> Verdict {
    let Some(rule) = match_rule(&config.rules, &id.remote).cloned() else {
        return Verdict {
            rule: None,
            expected_email: String::new(),
            expected_name: String::new(),
            ok: true,
            reason: "no rule matched remote — policy not enforced".to_string(),
        };
    };
    let (email, name) = expected_identity(&rule, id);
    if !id
        .email
        .to_lowercase()
        .ends_with(&rule.require_suffix.to_lowercase())
    {
        let reason = format!(
            "email {:?} must end with {:?}",
            id.email, rule.require_suffix
        );
        return Verdict {
            rule: Some(rule),
            expected_email: email,
            expected_name: name,
            ok: false,
            reason,
        };
    }
    if !rule.name_format.is_empty() && id.name != name {
        let reason = format!(
            "name {:?} must equal {:?} per name_format={:?}",
            id.name, name, rule.name_format
        );
        return Verdict {
            rule: Some(rule),
            expected_email: email,
            expected_name: name,
            ok: false,
            reason,
        };
    }
    Verdict {
        rule: Some(rule),
        expected_email: email,
        expected_name: name,
        ok: true,
        reason: "identity matches rule".to_string(),
    }
}
