// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Filesystem helpers shared by the quality gates.
//!
//! The Go original walks the tree with `filepath.Walk` and a hand-rolled set
//! of skip directories. This module keeps the same semantics but returns
//! paths relative to the project root, always with forward slashes, so the
//! findings are stable across platforms.

use std::fs;
use std::path::{Path, PathBuf};

/// Directories that no quality gate ever descends into.
pub const SKIP_DIRS: &[&str] = &[
    "vendor",
    "node_modules",
    "dist",
    "build",
    "coverage",
    "out",
    "bin",
    "reports",
    ".git",
    ".cache",
    ".turbo",
    ".next",
    ".codegraph",
    ".omo",
    "submodules",
    ".go-mod",
    "target",
];

/// Whether a directory name should be skipped entirely.
pub fn should_skip_dir(name: &str) -> bool {
    SKIP_DIRS.contains(&name)
}

/// Go's `filepath.Join(root, rel)`: concatenate with `/`, then `Clean`.
///
/// Unlike [`Path::join`](std::path::Path::join), a later absolute element
/// does not reset the result — Go's `Join` only cleans the joined string, so
/// `Join("/root", "/tmp/x.xml")` is `/root/tmp/x.xml`.
pub fn go_join(root: &Path, rel: &str) -> PathBuf {
    let mut joined = root.to_string_lossy().to_string();
    if !joined.ends_with('/') {
        joined.push('/');
    }
    joined.push_str(rel);
    PathBuf::from(go_clean(&joined))
}

/// Go's `path.Clean` for one slash-separated path.
fn go_clean(path: &str) -> String {
    let rooted = path.starts_with('/');
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => match segments.last() {
                Some(&last) if last != ".." => {
                    segments.pop();
                }
                Some(_) if !rooted => segments.push(".."),
                None if !rooted => segments.push(".."),
                _ => {}
            },
            other => segments.push(other),
        }
    }
    let mut cleaned = String::new();
    if rooted {
        cleaned.push('/');
    }
    cleaned.push_str(&segments.join("/"));
    if cleaned.is_empty() {
        ".".to_string()
    } else {
        cleaned
    }
}

/// A file discovered under the project root.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Path relative to the project root, using forward slashes.
    pub rel: String,
    /// Absolute path on disk.
    pub path: PathBuf,
}

impl Entry {
    /// The lowercased extension of the file, including the dot.
    pub fn extension(&self) -> String {
        self.path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
            .unwrap_or_default()
    }
}

/// Walks `root`, yielding every file that is not inside a skipped directory.
///
/// Symlinks are not followed: a link out of the tree would make the walk
/// non-terminating and the findings non-reproducible.
pub fn walk_files(root: &Path) -> Vec<Entry> {
    let mut out = Vec::new();
    walk_into(root, root, &mut out);
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

fn walk_into(root: &Path, dir: &Path, out: &mut Vec<Entry>) {
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for item in read.flatten() {
        let path = item.path();
        let file_type = match item.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let name = item.file_name().to_string_lossy().to_string();
        if file_type.is_dir() {
            if should_skip_dir(&name) {
                continue;
            }
            walk_into(root, &path, out);
        } else if file_type.is_file() {
            let rel = relative_slashed(root, &path);
            out.push(Entry { rel, path });
        }
    }
}

/// Renders `path` relative to `root` with forward slashes.
pub fn relative_slashed(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.to_string_lossy().replace('\\', "/")
}

/// Counts the lines of a text file the way the Go original does.
///
/// A trailing newline does not add an empty final line; a file without one
/// still counts its last line.
pub fn count_text_lines(data: &[u8]) -> usize {
    if data.is_empty() {
        return 0;
    }
    let mut lines = data.iter().filter(|b| **b == b'\n').count();
    if data[data.len() - 1] != b'\n' {
        lines += 1;
    }
    lines
}

/// Whether the file looks machine-generated and should not be held to the
/// human-written budgets.
pub fn is_generated_file(data: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&data[..data.len().min(4096)]).to_lowercase();
    if head.contains("code generated") && head.contains("do not edit") {
        return true;
    }
    const MARKERS: &[&str] = &[
        "@generated",
        "<auto-generated",
        "this file was generated",
        "automatically generated",
    ];
    MARKERS.iter().any(|m| head.contains(m))
}

/// Whether the file name is a dependency lock file.
pub fn is_dependency_lock_file(rel: &str) -> bool {
    let base = rel.rsplit('/').next().unwrap_or(rel).to_lowercase();
    matches!(
        base.as_str(),
        "package-lock.json" | "pnpm-lock.yaml" | "yarn.lock" | "bun.lock" | "bun.lockb"
    )
}

/// Whether the path looks like a database migration, which is exempt from the
/// declarative file-length budget.
pub fn is_migration_path(rel: &str) -> bool {
    let lower = rel.to_lowercase();
    const SEGMENTS: &[&str] = &["/migrations/", "/migration/", "/db/migrate/"];
    SEGMENTS.iter().any(|segment| {
        lower.contains(segment) || lower.starts_with(segment.trim_start_matches('/'))
    })
}

/// Strips a UTF-8 byte-order mark, if present.
pub fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// Reads a file, returning `None` when it cannot be read.
pub fn read_file(path: &Path) -> Option<Vec<u8>> {
    fs::read(path).ok()
}

/// Whether a path names an existing directory.
pub fn is_dir(path: &Path) -> bool {
    path.is_dir()
}

/// Whether a path names an existing file.
pub fn is_file(path: &Path) -> bool {
    path.is_file()
}

/// Resolves `path` against the current directory the way Go's
/// `filepath.Abs` does: a purely lexical join plus `Clean`, with no
/// symlink resolution, so the result matches the Go tool byte for byte.
pub fn absolute(path: &Path) -> Result<PathBuf, std::io::Error> {
    let mut out = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir()?
    };
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_lines_the_way_the_go_tool_does() {
        assert_eq!(count_text_lines(b""), 0);
        assert_eq!(count_text_lines(b"a\n"), 1);
        assert_eq!(count_text_lines(b"a\nb\n"), 2);
        assert_eq!(count_text_lines(b"a\nb"), 2);
    }

    #[test]
    fn go_join_follows_path_filepath() {
        assert_eq!(
            go_join(Path::new("/root"), "/tmp/x.xml"),
            PathBuf::from("/root/tmp/x.xml")
        );
        assert_eq!(
            go_join(Path::new("/root"), "../x.xml"),
            PathBuf::from("/x.xml")
        );
        assert_eq!(
            go_join(Path::new("/root"), "junit.xml"),
            PathBuf::from("/root/junit.xml")
        );
        assert_eq!(go_join(Path::new("/root"), ""), PathBuf::from("/root"));
    }

    #[test]
    fn recognises_generated_files() {
        assert!(is_generated_file(
            b"// Code generated by protoc. DO NOT EDIT."
        ));
        assert!(is_generated_file(b"// @generated\nfn main() {}"));
        assert!(!is_generated_file(b"fn main() {}"));
    }

    #[test]
    fn recognises_lock_files_and_migrations() {
        assert!(is_dependency_lock_file("web/package-lock.json"));
        assert!(!is_dependency_lock_file("web/package.json"));
        assert!(is_migration_path("db/migrations/001_init.sql"));
        assert!(is_migration_path("migrations/001_init.sql"));
        assert!(!is_migration_path("db/schema.sql"));
    }

    #[test]
    fn skips_build_directories() {
        assert!(should_skip_dir("vendor"));
        assert!(should_skip_dir("target"));
        assert!(!should_skip_dir("src"));
    }
}
