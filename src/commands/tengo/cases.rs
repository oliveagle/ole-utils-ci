// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tengo case discovery, mirroring `pkg/tengoqa.DiscoverCaseFiles`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Directories the sidecar walk never descends into.
const SKIP_DIRS: &[&str] = &[".git", "vendor", "node_modules", "reports"];

/// Finds the sidecar tool tests plus the conventional `tests/tengo` directory.
///
/// Explicit files and directories take precedence over discovery, exactly as
/// they do in the Go original.
pub fn discover(src: &str, requested: &[String]) -> Result<Vec<String>, String> {
    let root = crate::fsutil::absolute(Path::new(src)).map_err(|e| e.to_string())?;
    if !requested.is_empty() {
        let mut files = Vec::new();
        for path in requested {
            files.extend(expand(path)?);
        }
        return Ok(dedupe_sorted(&files));
    }
    let mut files = Vec::new();
    walk_root(&root, &mut files)?;
    let repo_tests = sibling_tests_dir(&root);
    if repo_tests.is_dir() {
        files.extend(expand_path(&repo_tests)?);
    }
    Ok(dedupe_sorted(&files))
}

/// `filepath.Dir(srcRoot)` joined with `tests/tengo`.
fn sibling_tests_dir(root: &Path) -> PathBuf {
    root.parent().unwrap_or(root).join("tests").join("tengo")
}

/// Mirrors `filepath.WalkDir` over the source root.
fn walk_root(root: &Path, files: &mut Vec<String>) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(root).map_err(|e| io_error("lstat", root, &e))?;
    if meta.is_dir() {
        walk_dir(root, root, files)
    } else {
        collect_case(root, files);
        Ok(())
    }
}

/// Walks one directory, skipping the conventional names the Go tool skips.
fn walk_dir(root: &Path, dir: &Path, files: &mut Vec<String>) -> Result<(), String> {
    if dir != root && should_skip(dir) {
        return Ok(());
    }
    for entry in read_sorted(dir)? {
        if entry.is_dir {
            walk_dir(root, &entry.path, files)?;
        } else {
            collect_case(&entry.path, files);
        }
    }
    Ok(())
}

/// Records `path` when it is a YAML file living under a `tests` directory.
fn collect_case(path: &Path, files: &mut Vec<String>) {
    if is_yaml(path) && path.to_string_lossy().contains("/tests/") {
        files.push(path.to_string_lossy().into_owned());
    }
}

/// Expands one `--cases` entry: a directory becomes every YAML file below it.
fn expand(requested: &str) -> Result<Vec<String>, String> {
    let path = Path::new(requested);
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("case path {requested}: {}", io_error("stat", path, &e)))?;
    if meta.is_dir() {
        return expand_path(path);
    }
    if !is_yaml(path) {
        return Err(format!("case file {requested} must be .yaml or .yml"));
    }
    Ok(vec![requested.to_string()])
}

/// Every YAML file at or below `dir`.
fn expand_path(dir: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for entry in read_sorted(dir)? {
        if entry.is_dir {
            files.extend(expand_path(&entry.path)?);
        } else if is_yaml(&entry.path) {
            files.push(entry.path.to_string_lossy().into_owned());
        }
    }
    Ok(files)
}

/// One dirent plus the directory bit, without ever following symlinks.
struct DirEntry {
    path: PathBuf,
    is_dir: bool,
}

/// The entries of `dir`, sorted by name the way Go's `os.ReadDir` sorts them.
fn read_sorted(dir: &Path) -> Result<Vec<DirEntry>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| io_error("open", dir, &e))?;
    let mut items: Vec<DirEntry> = entries
        .flatten()
        .map(|entry| DirEntry {
            is_dir: entry.file_type().is_ok_and(|kind| kind.is_dir()),
            path: entry.path(),
        })
        .collect();
    items.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(items)
}

/// Whether `path` names a directory the walk skips.
fn should_skip(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| SKIP_DIRS.contains(&name.to_string_lossy().as_ref()))
}

/// Whether `path` has a `.yaml` / `.yml` extension, case-insensitively.
fn is_yaml(path: &Path) -> bool {
    let Some(name) = path.file_name() else {
        return false;
    };
    name.to_string_lossy()
        .rsplit_once('.')
        .is_some_and(|(_, ext)| matches!(ext.to_lowercase().as_str(), "yaml" | "yml"))
}

/// Deduplicates by absolute path and sorts, the way Go's helper does.
fn dedupe_sorted(files: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    for file in files {
        if let Ok(abs) = crate::fsutil::absolute(Path::new(file)) {
            seen.insert(abs.to_string_lossy().into_owned());
        }
    }
    seen.into_iter().collect()
}

/// Go's `*os.PathError` wording: `op path: strerror`.
fn io_error(op: &str, path: &Path, error: &std::io::Error) -> String {
    format!("{op} {}: {}", path.display(), errno_text(error))
}

/// The lowercase `strerror` text Go prints for a raw errno.
fn errno_text(error: &std::io::Error) -> String {
    let text = error.to_string();
    let message = match text.rfind(" (os error ") {
        Some(index) => &text[..index],
        None => text.as_str(),
    };
    let mut chars = message.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_lowercase(), chars.as_str()),
        None => message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_a_missing_source_root_the_way_go_does() {
        let err = discover("definitely-missing-src-xyz", &[]).unwrap_err();
        assert!(
            err.starts_with("lstat ") && err.ends_with("no such file or directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn reports_a_missing_explicit_case_path() {
        let err = discover("src", &["nope.yaml".to_string()]).unwrap_err();
        assert_eq!(
            err,
            "case path nope.yaml: stat nope.yaml: no such file or directory"
        );
    }

    #[test]
    fn only_yaml_files_under_a_tests_directory_count() {
        assert!(is_yaml(Path::new("a/tests/case.yaml")));
        assert!(is_yaml(Path::new(".yaml")));
        assert!(!is_yaml(Path::new("a/tests/case.json")));
        assert!(should_skip(Path::new("a/vendor")));
        assert!(!should_skip(Path::new("a/src")));
    }
}
