// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `buf-lint` command: the proto naming conventions.
//!
//! The Go original compiles the protos with `protocompile` and inspects the
//! descriptors. This rewrite reads the `.proto` sources directly, which keeps
//! the gate dependency-free; the rules themselves are the same.

use std::path::Path;

use crate::config::Config;

mod check;

pub use check::{check_file, is_pascal, is_snake};

/// One naming violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtoIssue {
    /// 1-based line number.
    pub line: usize,
    /// The rule that was broken.
    pub rule: String,
    /// A one-line description.
    pub message: String,
}

/// The directories that hold the proto sources.
const PROTO_ROOTS: [&str; 4] = ["proto", "api", "protos", "buf"];

/// Finds the proto root of the project.
pub fn find_proto_root(root: &Path) -> Option<std::path::PathBuf> {
    for candidate in PROTO_ROOTS {
        let path = root.join(candidate);
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

/// Lists the proto files of the project.
pub fn list_proto_files(root: &Path) -> Vec<std::path::PathBuf> {
    let Some(proto_root) = find_proto_root(root) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    collect_proto_files(&proto_root, &mut files);
    files.sort();
    files
}

/// Collects the `.proto` files of one directory, recursively.
fn collect_proto_files(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_proto_files(&path, files);
        } else if path.extension().and_then(|e| e.to_str()) == Some("proto") {
            files.push(path);
        }
    }
}

/// The `buf-lint` command.
///
/// Go's `BufLint` writes its progress lines whatever `--output` says: the
/// command reports on stdout and returns its findings as an error, so the
/// output mode never changes what is printed.
pub fn buf_lint(cfg: &Config) -> Result<i32, String> {
    let root = cfg.root();
    println!("🔍 Running proto lint...");
    let Some(proto_root) = find_proto_root(root) else {
        println!("  ⚠ No proto configuration found, skipping");
        return Ok(0);
    };
    let files = list_proto_files(root);
    if files.is_empty() {
        println!("  No .proto files found");
        return Ok(0);
    }
    println!(
        "  Found {} proto files in {}",
        files.len(),
        proto_root.display()
    );
    let mut violations = Vec::new();
    for file in &files {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        let relative = file
            .strip_prefix(&proto_root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        for issue in check_file(&relative, &source) {
            violations.push(issue.message);
        }
    }
    if !violations.is_empty() {
        let lines: Vec<String> = violations.iter().map(|line| format!("  {line}")).collect();
        return Err(format!("lint violations:\n{}", lines.join("\n")));
    }
    println!("✅ Proto lint passed");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_the_package_against_the_directory() {
        let issues = check_file("api/v1/user.proto", "package api.v1;\n");
        assert!(issues.is_empty());
        let issues = check_file("api/v1/user.proto", "package wrong;\n");
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn checks_the_declaration_casing() {
        let issues = check_file("a.proto", "message user_profile {}\n");
        assert_eq!(issues.len(), 1);
        let issues = check_file("a.proto", "message UserProfile {}\n");
        assert!(issues.is_empty());
    }

    #[test]
    fn checks_the_field_casing() {
        let issues = check_file("a.proto", "message User {\n  string userName = 1;\n}\n");
        assert_eq!(issues.len(), 1);
        let issues = check_file("a.proto", "message User {\n  string user_name = 1;\n}\n");
        assert!(issues.is_empty());
    }
}
