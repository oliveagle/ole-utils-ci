// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Byte-for-byte comparison of two directory trees.
//!
//! Rust port of the Go original's `internal/treemirror.go` plus the
//! `tree_mirrors` half of `gatherVersionCheck`. Both roots are indexed by
//! the path relative to that root, then the union is walked: a path only on
//! the left is `missing-in-dst`, only on the right is `missing-in-src`, and
//! a path on both sides whose bytes differ is `content-diff`. Mirrors get
//! embedded into release binaries, so nothing is normalised — the bytes have
//! to be identical.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use walkdir::WalkDir;

use crate::commands::ci_recipe_util::go_errno;
use crate::lint_config::ProjectConfig;

use super::{Info, TreeMirrorCheck};

/// One difference between the two trees.
struct Diff {
    /// Why the two sides disagree.
    kind: &'static str,
    /// The path, relative to the compared roots.
    rel: String,
    /// What the entry is, in the form the message embeds.
    detail: String,
}

/// One walked path: what it is and, for a file, its bytes.
struct Entry {
    is_dir: bool,
    size: u64,
    bytes: Vec<u8>,
}

/// Indexes every path under `root`, keyed by the path relative to `root`.
fn read_tree(root: &Path) -> Result<BTreeMap<String, Entry>, String> {
    if root.as_os_str().is_empty() {
        return Err("empty root directory".to_string());
    }
    let metadata = std::fs::metadata(root)
        .map_err(|error| format!("stat {}: {}", root.display(), go_errno(&error)))?;
    if !metadata.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }
    let mut entries = BTreeMap::new();
    for walked in WalkDir::new(root) {
        let walked = walked.map_err(walk_error)?;
        let path = walked.path();
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let is_dir = walked.file_type().is_dir();
        let (size, bytes) = if is_dir {
            (0, Vec::new())
        } else {
            let info = std::fs::symlink_metadata(path)
                .map_err(|error| format!("lstat {}: {}", path.display(), go_errno(&error)))?;
            let bytes = std::fs::read(path)
                .map_err(|error| format!("open {}: {}", path.display(), go_errno(&error)))?;
            (info.len(), bytes)
        };
        entries.insert(
            relative.to_string_lossy().into_owned(),
            Entry {
                is_dir,
                size,
                bytes,
            },
        );
    }
    Ok(entries)
}

/// The wording Go's `filepath.WalkDir` hands back for a failed step.
fn walk_error(error: walkdir::Error) -> String {
    let path = error.path().map(|path| path.display().to_string());
    match (path, error.io_error()) {
        (Some(path), Some(io)) => format!("open {path}: {}", go_errno(io)),
        (_, Some(io)) => go_errno(io),
        (Some(path), None) => path,
        (None, None) => error.to_string(),
    }
}

/// Every difference between `src` and `dst`; an empty slice means identical.
fn compare(src: &Path, dst: &Path) -> Result<Vec<Diff>, String> {
    let left = read_tree(src).map_err(|error| format!("read src {}: {error}", src.display()))?;
    let right = read_tree(dst).map_err(|error| format!("read dst {}: {error}", dst.display()))?;
    let mut paths: BTreeSet<&str> = BTreeSet::new();
    for key in left.keys().chain(right.keys()) {
        paths.insert(key);
    }
    let mut diffs = Vec::new();
    for rel in paths {
        let src_entry = left.get(rel);
        let dst_entry = right.get(rel);
        if let (Some(src_entry), None) = (src_entry, dst_entry) {
            diffs.push(Diff {
                kind: "missing-in-dst",
                rel: rel.to_string(),
                detail: describe(src_entry),
            });
            continue;
        }
        if let (None, Some(dst_entry)) = (src_entry, dst_entry) {
            diffs.push(Diff {
                kind: "missing-in-src",
                rel: rel.to_string(),
                detail: describe(dst_entry),
            });
            continue;
        }
        if let (Some(src_entry), Some(dst_entry)) = (src_entry, dst_entry) {
            if src_entry.is_dir != dst_entry.is_dir {
                diffs.push(Diff {
                    kind: "content-diff",
                    rel: rel.to_string(),
                    detail: format!(
                        "type mismatch (src {}, dst {})",
                        kind_of(src_entry.is_dir),
                        kind_of(dst_entry.is_dir)
                    ),
                });
            } else if !src_entry.is_dir
                && (src_entry.size != dst_entry.size || src_entry.bytes != dst_entry.bytes)
            {
                // Go always reports the two sizes, even when only the bytes differ.
                diffs.push(Diff {
                    kind: "content-diff",
                    rel: rel.to_string(),
                    detail: format!("size {} != {}", src_entry.size, dst_entry.size),
                });
            }
        }
    }
    Ok(diffs)
}

/// What one entry is, in the short form a missing-entry note uses.
fn describe(entry: &Entry) -> String {
    if entry.is_dir {
        "directory".to_string()
    } else {
        format!("file, {} bytes", entry.size)
    }
}

/// `file` or `dir`, the way a type mismatch names the two sides.
fn kind_of(is_dir: bool) -> &'static str {
    if is_dir {
        "dir"
    } else {
        "file"
    }
}

/// Renders at most `max` entries on one line, the way Go does.
fn summarise(diffs: &[Diff], max: usize) -> String {
    if diffs.is_empty() {
        return "(no diff)".to_string();
    }
    let mut parts = Vec::new();
    for (index, diff) in diffs.iter().enumerate() {
        if index >= max {
            parts.push(format!("... (+{} more)", diffs.len() - max));
            break;
        }
        parts.push(format!("[{}] {} — {}", diff.kind, diff.rel, diff.detail));
    }
    parts.join("; ")
}

/// Validates one `tree_mirrors` entry and compares `src` against every dst.
pub(super) fn check(root: &Path, project: &ProjectConfig, info: &mut Info) -> Result<(), String> {
    for mirror in &project.tree_mirrors {
        if mirror.src.trim().is_empty() {
            return Err(
                "tree_mirrors entry with empty src — fix .devine-utils-ci.yaml".to_string(),
            );
        }
        if mirror.dsts.is_empty() {
            return Err(format!(
                "tree_mirrors src {:?} has no dsts — fix .devine-utils-ci.yaml",
                mirror.src
            ));
        }
        for dst in &mirror.dsts {
            if dst.trim().is_empty() {
                return Err(format!(
                    "tree_mirrors src {:?} has an empty dst — fix .devine-utils-ci.yaml",
                    mirror.src
                ));
            }
            check_one(root, mirror.src.as_str(), dst, info)?;
        }
    }
    Ok(())
}

/// Compares one declared `src` against one declared destination.
fn check_one(root: &Path, src: &str, dst: &str, info: &mut Info) -> Result<(), String> {
    let src_path = root.join(src);
    let dst_path = root.join(dst);
    let mut found = TreeMirrorCheck {
        src: src.to_string(),
        dst: dst.to_string(),
        ..TreeMirrorCheck::default()
    };
    if std::fs::metadata(&src_path).is_err() {
        found.note = format!("missing src {src:?}");
        info.tree_mirrors.push(found);
        return Err(format!(
            "tree mirror src {src:?} does not exist at {} — fix .devine-utils-ci.yaml or create it (drift is unsafe)",
            src_path.display()
        ));
    }
    if std::fs::metadata(&dst_path).is_err() {
        found.note = format!("missing dst {dst:?}");
        info.tree_mirrors.push(found);
        return Err(format!(
            "tree mirror dst {dst:?} does not exist at {} — run the project's sync command (e.g. make sync-skill-bundle); drift is unsafe",
            dst_path.display()
        ));
    }
    found.exists = true;
    let diffs = compare(&src_path, &dst_path)
        .map_err(|error| format!("compare tree mirror {src} vs {dst}: {error}"))?;
    if !diffs.is_empty() {
        let note = format!("drift: {}", summarise(&diffs, 5));
        found.note = note.clone();
        info.tree_mirrors.push(found);
        return Err(format!(
            "tree mirror src {src:?} is out of sync with dst {dst:?} — {note}"
        ));
    }
    found.matched = true;
    info.tree_mirrors.push(found);
    Ok(())
}

#[cfg(test)]
mod tests;
