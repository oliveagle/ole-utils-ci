// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The ALIGN verdict: how closely the newest tag sits on the project heads.

/// How closely the newest tag's commit sits on top of the project branches.
///
/// It drives both the colour and the digit rendered in the ALIGN column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignKind {
    /// No data: no tag, or the tag's commit SHA is unknown.
    Empty,
    /// The tag exists but main HEAD is on a different commit — drift.
    Red,
    /// main HEAD == tag commit: the release sits on main.
    Two,
    /// main HEAD == release HEAD == tag commit: the strict model, synced.
    Three,
}

/// The strict-model release branch name for a tag, e.g. `v0.2.9` → `release`.
///
/// Empty when the tag does not look like a release tag, so the caller never
/// polls `/repository/branches/<junk>` for a non-conforming project.
///
/// This is a pure function — no GitLab call. Whether the branch exists is
/// settled by the fetch.
pub fn release_branch_name(tag_name: &str) -> String {
    let Some(rest) = tag_name.strip_prefix('v') else {
        return String::new();
    };
    if rest.is_empty() {
        return String::new();
    }
    // Require at least major.minor and accept an optional patch or
    // pre-release suffix. The branch itself is always the single fixed
    // release branch the convention mandates.
    let mut has_dot = false;
    for (index, ch) in rest.char_indices() {
        if ch.is_ascii_digit() {
            continue;
        }
        if ch == '.' {
            has_dot = true;
            continue;
        }
        if ch == '-' && index > 0 && has_dot {
            return "release".to_string();
        }
        return String::new();
    }
    if has_dot {
        return "release".to_string();
    }
    String::new()
}

/// Derives the verdict for one row.
///
/// Both inputs are SHA strings; an empty one means "not fetched". The
/// comparison runs on the short SHA prefix, the way GitLab's web UI shows
/// it, so a long SHA from one endpoint still matches a short one from
/// another.
pub fn classify_align(main_sha: &str, release_sha: &str, tag_sha: &str) -> AlignKind {
    if tag_sha.is_empty() {
        return AlignKind::Empty;
    }
    let main_ok = !main_sha.is_empty();
    let release_ok = !release_sha.is_empty();
    let main_aligned = main_ok && short_sha(main_sha) == short_sha(tag_sha);
    let release_aligned = release_ok && short_sha(release_sha) == short_sha(tag_sha);
    if main_aligned && release_aligned {
        return AlignKind::Three;
    }
    // The release sits directly on main only when the project clearly does
    // not use a release branch. A present but disagreeing release SHA means
    // the tag drifted past it, which is drift.
    if main_aligned && !release_ok {
        return AlignKind::Two;
    }
    AlignKind::Red
}

/// The first eight hex characters of a SHA, or the whole string when it is
/// shorter.
fn short_sha(value: &str) -> &str {
    if value.len() <= 8 {
        return value;
    }
    value.get(..8).unwrap_or(value)
}

#[cfg(test)]
mod tests;
