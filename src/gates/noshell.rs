// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `no-shell-exec` gate: production code must not shell out to `bash` or
//! `sh`.
//!
//! Test files are deliberately excluded: tests may use shell fixtures without
//! making the shipped binary depend on a shell. The Go original parsed Go
//! with `go/ast`; this rewrite scans the source text of both Go and Rust, so
//! a Rust project gets the same protection.

use std::path::Path;

use crate::fsutil;
use crate::gates::focused;
use crate::lint_config;
use crate::output::Diagnostic;

/// The directories scanned by default.
pub const DEFAULT_DIRS: [&str; 2] = ["cmd", "internal"];

/// The shell programs that are not allowed.
pub const FORBIDDEN_SHELLS: [&str; 3] = ["bash", "sh", "zsh"];

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellFinding {
    /// Path relative to the project root.
    pub path: String,
    /// 1-based line number.
    pub line: usize,
    /// The rule that was broken.
    pub rule: String,
    /// A one-line description.
    pub message: String,
}

/// Scans the given directories for shell execution.
pub fn scan(
    root: &Path,
    dirs: &[String],
    forbidden_paths: &[String],
) -> (Vec<ShellFinding>, usize) {
    let mut findings = Vec::new();
    let mut scanned = 0usize;
    for rel_dir in dirs {
        let dir = root.join(rel_dir);
        if !dir.is_dir() {
            continue;
        }
        for entry in fsutil::walk_files(&dir) {
            let ext = entry.extension();
            if ext != ".go" && ext != ".rs" {
                continue;
            }
            if entry.rel.ends_with("_test.go") || entry.rel.ends_with("_test.rs") {
                continue;
            }
            scanned += 1;
            findings.extend(scan_file(&entry));
        }
    }
    for rel in forbidden_paths {
        if root.join(rel).exists() {
            findings.push(ShellFinding {
                path: rel.clone(),
                line: 0,
                rule: "no-shell/forbidden-file".to_string(),
                message: "forbidden shell implementation file exists".to_string(),
            });
        }
    }
    (findings, scanned)
}

fn scan_file(entry: &fsutil::Entry) -> Vec<ShellFinding> {
    let mut findings = Vec::new();
    let Some(data) = fsutil::read_file(&entry.path) else {
        return findings;
    };
    let source = String::from_utf8_lossy(&data);
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }
        let Some(shell) = detect_shell(trimmed) else {
            continue;
        };
        findings.push(ShellFinding {
            path: entry.rel.clone(),
            line: index + 1,
            rule: "no-shell/exec".to_string(),
            message: format!("production code executes {shell:?}"),
        });
    }
    findings
}

/// Detects a shell invocation in one line of source.
fn detect_shell(line: &str) -> Option<&'static str> {
    let trimmed = line.trim();
    if trimmed.starts_with("//") || trimmed.starts_with('#') {
        return None;
    }
    let lowered = trimmed.to_lowercase();
    for shell in FORBIDDEN_SHELLS {
        if !lowered.contains(shell) {
            continue;
        }
        let looks_like_exec = lowered.contains("exec.command")
            || lowered.contains("exec::command")
            || lowered.contains("command::new")
            || lowered.contains("process::command")
            || lowered.contains("look_path")
            || lowered.contains("lookpath")
            || lowered.contains("which::");
        if looks_like_exec {
            return Some(shell);
        }
    }
    None
}

/// Runs the gate the way the Go command does: a private flag set, TOON
/// output by default, exit 2 for a refused command line and exit 1 for a
/// finding.
pub fn focused_command(args: &[String]) -> Result<i32, String> {
    let root = focused::root_default(args);
    let config = lint_config::load(&root)?;
    let resolved = lint_config::resolve(&config);
    let mut fs = focused::base(&root, args);
    fs.str_flag("dirs", &resolved.no_shell.dirs.join(","));
    fs.str_flag(
        "forbidden-paths",
        &resolved.no_shell.forbidden_paths.join(","),
    );
    fs.describe("dirs", "comma-separated production Go directories");
    fs.describe(
        "forbidden-paths",
        "comma-separated shell files that must not exist",
    );
    let flags = match focused::finish("no-shell-exec", args, fs) {
        Ok(flags) => flags,
        Err(failure) => return failure.into_result(),
    };
    let dirs = split_csv(&flags.string("dirs"));
    let forbidden = split_csv(&flags.string("forbidden-paths"));
    let (findings, scanned) = scan(flags.root(), &dirs, &forbidden);
    let diagnostics = findings.iter().map(to_diagnostic).collect::<Vec<_>>();
    let extra = serde_json::json!({ "dirs": dirs, "files_scanned": scanned });
    focused::render("no-shell-exec", &diagnostics, Some(extra), flags.mode())?;
    Ok(0)
}

/// One finding as the shared report sees it.
fn to_diagnostic(finding: &ShellFinding) -> Diagnostic {
    Diagnostic::error(
        finding.path.clone(),
        finding.rule.clone(),
        finding.message.clone(),
    )
    .at_line(finding.line)
}

/// Splits a comma-separated flag the way Go does: trim each part and drop
/// the empty ones.
fn split_csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_shell_execution() {
        assert_eq!(
            detect_shell("exec.Command(\"bash\", \"-c\", s)"),
            Some("bash")
        );
        assert_eq!(detect_shell("Command::new(\"sh\")"), Some("sh"));
        assert_eq!(detect_shell("exec.LookPath(\"bash\")"), Some("bash"));
    }

    #[test]
    fn ignores_comments_and_unrelated_text() {
        assert_eq!(detect_shell("// exec.Command(\"bash\")"), None);
        assert_eq!(detect_shell("let shell = \"bash\";"), None);
    }
}
