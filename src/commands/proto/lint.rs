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
use crate::output::{Diagnostic, Report};

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

/// Checks the naming conventions of one proto file.
///
/// `relative` is the path of the file relative to the proto root; the package
/// name has to match its directory.
pub fn check_file(relative: &str, source: &str) -> Vec<ProtoIssue> {
    let mut issues = Vec::new();
    let mut in_message = 0usize;
    for (index, line) in source.lines().enumerate() {
        let number = index + 1;
        let trimmed = line.trim();
        let trimmed = trimmed.strip_prefix("//").unwrap_or(trimmed).trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("package ") {
            let package = rest.trim().trim_end_matches(';').trim();
            check_package(relative, package, number, &mut issues);
            continue;
        }
        if trimmed.starts_with("message ") {
            in_message += 1;
            check_pascal(relative, trimmed, "message", number, &mut issues);
            continue;
        }
        if trimmed.starts_with("enum ") {
            check_pascal(relative, trimmed, "enum", number, &mut issues);
            continue;
        }
        if trimmed.starts_with("service ") {
            check_pascal(relative, trimmed, "service", number, &mut issues);
            continue;
        }
        if trimmed.starts_with("rpc ") {
            check_pascal(relative, trimmed, "rpc", number, &mut issues);
            continue;
        }
        if in_message > 0 && trimmed.ends_with(';') {
            check_field(relative, trimmed, number, &mut issues);
        }
        if trimmed == "}" {
            in_message = in_message.saturating_sub(1);
        }
    }
    issues
}

/// Checks that the package name matches the directory of the file.
fn check_package(relative: &str, package: &str, line: usize, issues: &mut Vec<ProtoIssue>) {
    let dir = std::path::Path::new(relative)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let expected = dir.replace('/', ".");
    if expected.is_empty() || expected == "." {
        return;
    }
    if package == expected || expected.ends_with(&format!(".{package}")) || package.starts_with('.')
    {
        return;
    }
    issues.push(ProtoIssue {
        line,
        rule: "PACKAGE_DIRECTORY_MATCH".to_string(),
        message: format!("package {package:?} should match directory path {dir:?}"),
    });
}

/// Checks that a declaration name is PascalCase.
fn check_pascal(
    relative: &str,
    line: &str,
    kind: &str,
    number: usize,
    issues: &mut Vec<ProtoIssue>,
) {
    let Some(name) = declaration_name(line, kind) else {
        return;
    };
    if is_pascal(&name) {
        return;
    }
    issues.push(ProtoIssue {
        line: number,
        rule: format!("{}_CASING", kind.to_uppercase()),
        message: format!("{relative}: {kind} {name:?} should be PascalCase"),
    });
}

/// Checks that a field name is snake_case.
fn check_field(relative: &str, line: &str, number: usize, issues: &mut Vec<ProtoIssue>) {
    let Some((left, _)) = line.rsplit_once('=') else {
        return;
    };
    let Some(name) = left.split_whitespace().next_back() else {
        return;
    };
    let name = name.trim_matches('"');
    if name.is_empty() || is_snake(name) {
        return;
    }
    issues.push(ProtoIssue {
        line: number,
        rule: "FIELD_LOWER_SNAKE_CASE".to_string(),
        message: format!("{relative}: field {name:?} should be snake_case"),
    });
}

/// The name of a declaration, for example `message Foo {`.
fn declaration_name(line: &str, kind: &str) -> Option<String> {
    let rest = line.strip_prefix(kind)?.trim();
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Whether a name is PascalCase.
pub fn is_pascal(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_uppercase() => !name.contains('_'),
        _ => false,
    }
}

/// Whether a name is snake_case.
pub fn is_snake(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| !c.is_uppercase())
}

/// The `buf-lint` command.
pub fn buf_lint(cfg: &Config) -> Result<i32, String> {
    let root = cfg.root();
    let Some(proto_root) = find_proto_root(root) else {
        println!("No proto directory found — skipping.");
        return Ok(0);
    };
    let files = list_proto_files(root);
    let mut report = Report::new("buf-lint");
    let mut total = 0usize;
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
            total += 1;
            report.add(
                Diagnostic::error(relative.clone(), issue.rule.clone(), issue.message.clone())
                    .at_line(issue.line),
            );
        }
    }
    report.extra = Some(serde_json::json!({ "files": files.len(), "issues": total }));
    if report.ok {
        report.summary = format!("buf-lint: {} file(s) clean", files.len());
        print!("{}", report.render_text());
        Ok(0)
    } else {
        report.summary = format!("buf-lint: {total} issue(s) found");
        print!("{}", report.render_text());
        Err(report.summary)
    }
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
