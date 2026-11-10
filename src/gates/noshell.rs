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

use crate::config::Config;
use crate::fsutil;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};

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

/// Runs the gate and returns its result.
pub fn run(cfg: &Config) -> Result<GateResult, String> {
    let dirs: Vec<String> = DEFAULT_DIRS.iter().map(|s| s.to_string()).collect();
    let (findings, scanned) = scan(cfg.root(), &dirs, &[]);
    let mut report = Report::new("no-shell-exec");
    report.extra = Some(serde_json::json!({
        "files_scanned": scanned,
        "dirs": dirs,
    }));
    for f in &findings {
        report.add(
            Diagnostic::error(f.path.clone(), f.rule.clone(), f.message.clone()).at_line(f.line),
        );
    }
    if report.ok {
        report.summary = format!("no-shell-exec: scanned {scanned} files, no shell execution");
        Ok(GateResult::finish(report))
    } else {
        report.summary = format!("no-shell-exec: {} finding(s)", findings.len());
        Ok(GateResult::failed(
            "no-shell-exec",
            report.summary.clone(),
            report,
        ))
    }
}

/// The human-readable body printed when `--output` is not set.
pub fn text_body(result: &GateResult) -> String {
    let mut body = String::from("🚫 Checking for shell execution in production code...\n");
    for d in &result.report.diagnostics {
        body.push_str(&format!("  ✗ {}:{}: {}\n", d.path, d.line, d.message));
    }
    if result.passed() {
        body.push_str("✅ No shell execution found\n");
    } else {
        body.push_str(&format!(
            "✗ {} shell execution(s) found\n",
            result.report.diagnostics.len()
        ));
    }
    body
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
