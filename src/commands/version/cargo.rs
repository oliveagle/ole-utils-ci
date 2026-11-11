// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Keeping the `Cargo.toml` version in sync with the `VERSION` file.
//!
//! The file is edited line by line instead of being parsed and re-emitted,
//! so the formatting, the comments and the ordering of the manifest survive
//! the change.

use std::path::Path;

/// Reads the `package.version` field of the manifest.
pub fn read_version(root: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
    package_field(&raw, "version")
}

/// Writes a new `package.version` into the manifest.
///
/// Returns `Ok(false)` when the manifest has no `version` field to update.
pub fn write_version(root: &Path, version: &str) -> Result<bool, String> {
    let path = root.join("Cargo.toml");
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        // Go has no manifest to keep in step, and a project without one is
        // not an error: there is simply nothing to sync.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("failed to read Cargo.toml: {error}")),
    };
    let updated = set_package_field(&raw, "version", version);
    match updated {
        Some(text) => {
            std::fs::write(&path, text).map_err(|e| format!("failed to write Cargo.toml: {e}"))?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Reads one field of the `[package]` table.
fn package_field(raw: &str, field: &str) -> Option<String> {
    let mut in_package = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(value) = field_value(trimmed, field) {
            return Some(value);
        }
    }
    None
}

/// Replaces one field of the `[package]` table, keeping the rest of the file.
fn set_package_field(raw: &str, field: &str, value: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut in_package = false;
    let mut replaced = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
        } else if in_package && !replaced {
            if let Some(prefix) = field_prefix(trimmed, field) {
                out.push_str(&line[..line.len() - trimmed.len()]);
                out.push_str(&format!("{prefix} \"{value}\"\n"));
                replaced = true;
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    if replaced {
        Some(out)
    } else {
        None
    }
}

/// The prefix of a `field = ...` line, when the line sets `field`.
fn field_prefix<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    let (key, _) = line.split_once('=')?;
    if key.trim() != field {
        return None;
    }
    Some(&line[..line.find('=')? + 1])
}

/// Reads the quoted value of a `field = "value"` line.
fn field_value(line: &str, field: &str) -> Option<String> {
    let (key, value) = line.split_once('=')?;
    if key.trim() != field {
        return None;
    }
    let value = value.trim();
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    Some(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_package_version() {
        let raw = "[package]\nname = \"x\"\nversion = \"1.2.3\"\n\n[dependencies]\nserde = \"1\"\n";
        assert_eq!(package_field(raw, "version").as_deref(), Some("1.2.3"));
    }

    #[test]
    fn replaces_only_the_package_version() {
        let raw =
            "[package]\nname = \"x\"\nversion = \"1.2.3\"\n\n[dependencies]\nversion = \"9\"\n";
        let updated = set_package_field(raw, "version", "2.0.0").unwrap();
        assert!(updated.contains("version = \"2.0.0\""));
        assert!(updated.contains("[dependencies]\nversion = \"9\""));
    }
}
