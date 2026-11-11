// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The tag invariants `release-check` enforces.
//!
//! A stable tag has to sit on the release line *and* carry the marker
//! `release-finish` writes, so a hand-made `git tag -a` can never become a
//! release (ADR 0001). Release candidates are exempt from the marker: they
//! are pre-release artefacts a staging pipeline may push by hand.

use std::path::Path;

use crate::git;
use crate::version::Version;

use super::super::RELEASE_TAG_MARKER;

/// Go's `checkTagOnRef`: the tag's commit must be an ancestor of `ref_name`.
///
/// The check is fail-closed — a missing ref or a failed git call is an
/// error, never a silent pass — so a release can never ship from a commit
/// that is not on the release line.
pub(super) fn on_ref(
    root: &Path,
    tag: &str,
    tag_commit: &str,
    ref_name: &str,
) -> Result<(), String> {
    if git::run_go(root, &["rev-parse", "--verify", ref_name]).is_err() {
        return Err(format!(
            "cannot verify tag {tag} is on {ref_name}: {ref_name} not available — \
             run `git fetch origin` first"
        ));
    }
    let merge_base = git::run_go(root, &["merge-base", tag_commit, ref_name])
        .map_err(|error| format!("merge-base check for tag {tag} vs {ref_name}: {error}"))?;
    let merge_base = merge_base.trim();
    if merge_base == tag_commit {
        return Ok(());
    }
    Err(format!(
        "tag {tag} not on {ref_name} — release tags must be pushed from a commit on \
         {ref_name}. Merge the feature branch into {ref_name} first, then re-tag \
         (tag commit: {tag_commit}, merge-base: {merge_base})"
    ))
}

/// Go's `checkTagMarker`: a stable tag must name the tool that made it.
pub(super) fn marker(root: &Path, tag: &str) -> Result<(), String> {
    if is_rc(tag) {
        return Ok(());
    }
    let contents = git::run_go(root, &["tag", "-l", "--format=%(contents)", tag])
        .map_err(|error| format!("cannot read tag message for {tag}: {error}"))?;
    if contents.contains(RELEASE_TAG_MARKER) {
        return Ok(());
    }
    Err(format!(
        "tag {tag} was NOT created by `devine-utils-ci release-finish` — \
         manual stable tags are prohibited (ADR 0001). Delete this tag and re-release:\n  \
         git push origin :refs/tags/{tag}\n  git tag -d {tag}\n  \
         # on main, after the bump commit is merged:\n  \
         devine-utils-ci release-finish"
    ))
}

/// Go's `checkLocalTagOnMain`: an existing tag for this version must already
/// be reachable from `origin/main`, and must carry the marker.
pub(super) fn local_on_main(root: &Path, expected_tag: &str) -> Result<(), String> {
    let Ok(output) = git::run_go(root, &["rev-list", "-n", "1", expected_tag]) else {
        return Ok(()); // the tag does not exist yet — nothing to enforce
    };
    let tag_commit = output.trim();
    if tag_commit.is_empty() {
        return Ok(());
    }
    on_ref(root, expected_tag, tag_commit, "origin/main")?;
    marker(root, expected_tag)
}

/// Whether `tag` names a release candidate, the lane exempt from the marker.
fn is_rc(tag: &str) -> bool {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    Version::parse(version).is_ok_and(|parsed| parsed.is_rc)
}
