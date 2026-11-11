// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The naming rules behind `buf-lint`.

use super::ProtoIssue;

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
        message: format!(
            "{relative}: package {package:?} should match directory path {dir:?} (PACKAGE_DIRECTORY_MATCH)"
        ),
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
