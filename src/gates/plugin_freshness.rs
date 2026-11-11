// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `plugin-freshness` gate: a built plugin `.so` must not be older than
//! the source it was built from.
//!
//! Each directory under the plugin source root is one tool. Its identity
//! comes from `tool.jsonc`; the matching `.so` lives under the category
//! directory, with a flat layout as the fallback. A `.so` that has never been
//! built is a skip, not a failure: a fresh checkout before the first plugin
//! build must stay green.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::fsutil;
use crate::gates::focused;
use crate::lint_config;
use crate::output::Diagnostic;
use crate::static_regex;

/// Runs the gate the way the Go command does: a private flag set, TOON
/// output by default, exit 2 for a refused command line and exit 1 for a
/// stale plugin.
pub fn focused_command(args: &[String]) -> Result<i32, String> {
    let root = focused::root_default(args);
    let config = lint_config::load(&root)?;
    let resolved = lint_config::resolve(&root, &config);
    let mut fs = focused::base(&root, args);
    fs.str_flag("source-dir", &resolved.plugin.source_dir);
    fs.str_flag("plugins-dir", &resolved.plugin.plugins_dir);
    fs.describe("source-dir", "plugin source root");
    fs.describe("plugins-dir", "built plugin root");
    let flags = match focused::finish("plugin-freshness", args, fs) {
        Ok(flags) => flags,
        Err(failure) => return failure.into_result(),
    };
    let source_dir = flags.string("source-dir");
    let plugins_dir = flags.string("plugins-dir");
    let (findings, extra) = check(flags.root(), &source_dir, &plugins_dir)?;
    focused::render("plugin-freshness", &findings, Some(extra), flags.mode())?;
    Ok(0)
}

/// Checks every plugin source tree against its built `.so`.
///
/// Returns the stale findings and the `checked` / `skipped` / `stale`
/// counters the report carries.
pub(crate) fn check(
    root: &Path,
    source_dir: &str,
    plugins_dir: &str,
) -> Result<(Vec<Diagnostic>, serde_json::Value), String> {
    let src_root = root.join(source_dir);
    let plugin_root = root.join(plugins_dir);
    let entries = match std::fs::read_dir(&src_root) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((
                Vec::new(),
                serde_json::json!({ "checked": 0, "skipped": 0, "stale": 0 }),
            ));
        }
        Err(e) => return Err(format!("open {}: {e}", src_root.display())),
    };
    let mut tools = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("open {}: {e}", src_root.display()))?;
        if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            tools.push(entry);
        }
    }
    tools.sort_by_key(|entry| entry.file_name());

    let mut findings = Vec::new();
    let mut checked = 0usize;
    let mut skipped = 0usize;
    for tool in tools {
        let src_dir = src_root.join(tool.file_name());
        let name = tool.file_name().to_string_lossy().to_string();
        let (tool_id, category, has_manifest) = plugin_identity(&src_dir, &name)?;
        let so_path = locate_plugin(&plugin_root, &tool_id, &category, has_manifest);
        let metadata = match std::fs::metadata(&so_path) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                skipped += 1;
                continue;
            }
            Err(e) => return Err(format!("stat {}: {e}", so_path.display())),
        };
        checked += 1;
        let built_at = metadata
            .modified()
            .map_err(|e| format!("stat {}: {e}", so_path.display()))?;
        if let Some(newer) = first_newer(&src_dir, built_at) {
            findings.push(Diagnostic::error(
                relative(root, &so_path),
                "plugin-freshness/stale",
                format!(
                    "plugin .so is older than source file {} (tool={tool_id}, category={category})",
                    relative(root, &newer)
                ),
            ));
        }
    }
    findings.sort_by(|a, b| a.path.cmp(&b.path));
    let extra = serde_json::json!({
        "checked": checked,
        "plugins_dir": plugins_dir,
        "skipped": skipped,
        "source_dir": source_dir,
        "stale": findings.len(),
    });
    Ok((findings, extra))
}

/// Reads a tool's identity from its `tool.jsonc` manifest.
fn plugin_identity(src_dir: &Path, fallback: &str) -> Result<(String, String, bool), String> {
    let manifest = src_dir.join("tool.jsonc");
    let data = match std::fs::read(&manifest) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((fallback.to_string(), "misc".to_string(), false))
        }
        Err(e) => return Err(format!("open {}: {e}", manifest.display())),
    };
    let text = String::from_utf8_lossy(&data);
    let id = capture(r#""id"\s*:\s*"([^"]+)""#, &text).unwrap_or_else(|| fallback.to_string());
    let category =
        capture(r#""category"\s*:\s*"([^"]+)""#, &text).unwrap_or_else(|| "misc".to_string());
    Ok((id, category, true))
}

/// The `.so` a tool builds: the category layout first, the flat one as the
/// fallback when only that exists.
fn locate_plugin(root: &Path, tool_id: &str, category: &str, has_manifest: bool) -> PathBuf {
    if !has_manifest {
        return root.join(format!("{tool_id}.so"));
    }
    let nested = root.join(category).join(format!("{tool_id}.so"));
    if nested.exists() {
        return nested;
    }
    let flat = root.join(format!("{tool_id}.so"));
    if flat.exists() {
        return flat;
    }
    nested
}

/// The first file, in walk order, that is newer than `built_at`.
fn first_newer(dir: &Path, built_at: SystemTime) -> Option<PathBuf> {
    let mut entries: Vec<std::fs::DirEntry> = std::fs::read_dir(dir).ok()?.flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if let Some(found) = first_newer(&path, built_at) {
                return Some(found);
            }
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if modified > built_at {
            return Some(path);
        }
    }
    None
}

/// The first capture group of `pattern`, if it matches.
fn capture(pattern: &str, text: &str) -> Option<String> {
    let re = static_regex::compile(pattern);
    re.captures(text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// `path` relative to the project root, with forward slashes.
fn relative(root: &Path, path: &Path) -> String {
    fsutil::relative_slashed(root, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_source_tree_reports_the_empty_counters() {
        let root = Path::new("/nonexistent-plugin-root-for-tests");
        let (findings, extra) = check(root, "src/golang", "plugins").expect("check");
        assert!(findings.is_empty());
        assert_eq!(extra["checked"], 0);
        assert_eq!(extra["skipped"], 0);
        assert_eq!(extra["stale"], 0);
        assert!(extra.get("source_dir").is_none());
    }

    #[test]
    fn a_tool_without_a_manifest_uses_the_directory_name() {
        let dir = tempfile::tempdir().expect("temp dir");
        let (id, category, manifest) = plugin_identity(dir.path(), "fallback").expect("identity");
        assert_eq!(id, "fallback");
        assert_eq!(category, "misc");
        assert!(!manifest);

        std::fs::write(
            dir.path().join("tool.jsonc"),
            r#"{"id":"thing","category":"cmd"}"#,
        )
        .expect("manifest");
        let (id, category, manifest) = plugin_identity(dir.path(), "fallback").expect("identity");
        assert_eq!(id, "thing");
        assert_eq!(category, "cmd");
        assert!(manifest);
    }

    #[test]
    fn the_flat_layout_is_the_fallback() {
        let root = Path::new("/tmp");
        assert_eq!(
            locate_plugin(root, "thing", "misc", false),
            root.join("thing.so")
        );
        assert_eq!(
            locate_plugin(root, "thing", "cmd", true),
            root.join("cmd").join("thing.so")
        );
    }
}
