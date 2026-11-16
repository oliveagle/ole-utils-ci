// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The Python source gates that need no interpreter.
//!
//! `no-shell-exec` is the Python side of the Go gate of the same name: a
//! library that shells out is a library that runs whatever the machine's
//! `sh` happens to be, and a pipeline that concatenates a variable into it
//! runs whatever that variable happens to be. `deps` is the other half of
//! reproducible builds — an unpinned requirement is an input nobody chose.
//!
//! Both read the tree that [`crate::fsutil::walk_files`] yields, so the
//! project's `ignore` configuration applies to them like any other gate.

use crate::config::Config;
use crate::fsutil;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};

/// The extensions the shell-exec scan reads. `.pyi` holds signatures only,
/// but a stub can still call one of these, and it costs nothing to read.
const SOURCE_EXTENSIONS: [&str; 2] = [".py", ".pyi"];

/// The calls that hand a string to a shell, or ask a process to start one.
///
/// The list is deliberately short and unambiguous: each of these *always*
/// runs a command, so a hit is never a false positive about intent. A hit
/// inside a docstring is still worth a reader's attention, which is why the
/// gate does not try to parse Python to decide otherwise.
pub const SHELL_CALLS: [&str; 5] = [
    "os.system(",
    "os.popen(",
    "shell=True",
    "subprocess.getoutput(",
    "subprocess.getstatusoutput(",
];

/// One call to the shell found in the sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCall {
    /// Path relative to the project root.
    pub path: String,
    /// 1-based line number.
    pub line: usize,
    /// The call that was found.
    pub call: String,
}

/// The code part of a line: a `#` comment is dropped, quotes are not parsed.
fn code_part(line: &str) -> &str {
    match line.find('#') {
        Some(idx) => &line[..idx],
        None => line,
    }
}

/// Scans one source file's text for shell calls.
pub fn scan_source(rel: &str, text: &str) -> Vec<ShellCall> {
    let mut found = Vec::new();
    for (idx, raw) in text.lines().enumerate() {
        let code = code_part(raw);
        for call in SHELL_CALLS {
            if code.contains(call) {
                found.push(ShellCall {
                    path: rel.to_string(),
                    line: idx + 1,
                    call: call.trim_end_matches('(').to_string(),
                });
                break;
            }
        }
    }
    found
}

/// The `python-no-shell-exec` gate.
pub fn no_shell_exec(cfg: &Config) -> Result<i32, String> {
    let qcfg = crate::quality_config::QualityConfig::load(cfg.root())?.unwrap_or_default();
    let mut findings = Vec::new();
    let mut scanned = 0usize;
    for entry in fsutil::walk_files(cfg.root()) {
        if !SOURCE_EXTENSIONS.contains(&entry.extension().as_str()) {
            continue;
        }
        if qcfg.should_ignore(&entry.rel, "no-shell-exec") {
            continue;
        }
        scanned += 1;
        let Some(data) = fsutil::read_file(&entry.path) else {
            continue;
        };
        findings.extend(scan_source(&entry.rel, &String::from_utf8_lossy(&data)));
    }

    let mut report = Report::new("python-no-shell-exec");
    report.extra = Some(serde_json::json!({
        "files_scanned": scanned,
        "calls": SHELL_CALLS,
    }));
    for finding in &findings {
        report.add(
            Diagnostic::error(
                finding.path.clone(),
                "python.shell-exec",
                format!("{} hands a string to a shell", finding.call),
            )
            .at_line(finding.line),
        );
    }
    if report.ok {
        report.summary = format!("python-no-shell-exec: {scanned} file(s), no shell calls");
        return Ok(crate::gates::emit(cfg, &GateResult::finish(report)));
    }
    report.summary = format!(
        "python-no-shell-exec: {} shell call(s) in {scanned} file(s)",
        findings.len()
    );
    let text = format!(
        "python-no-shell-exec found {} call(s) that start a shell:\n{}",
        findings.len(),
        findings
            .iter()
            .map(|f| format!("  ✗ {}:{}: {}", f.path, f.line, f.call))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let result = GateResult::failed("python-no-shell-exec", report.summary.clone(), report)
        .failing(text, format!("{} shell call(s)", findings.len()));
    Ok(crate::gates::emit(cfg, &result))
}

/// One requirement that does not pin a version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unpinned {
    /// Path relative to the project root.
    pub path: String,
    /// 1-based line number.
    pub line: usize,
    /// The requirement as it was written.
    pub requirement: String,
}

/// Whether a requirement line pins its package to an exact version.
///
/// `name==1.2.3` is a pin. `name`, `name>=1`, `name~=1.2`, `name[extra]` and
/// `name @ https://…` are not: each one lets the resolver choose.
pub fn is_pinned(requirement: &str) -> bool {
    let body = requirement.split(';').next().unwrap_or(requirement).trim();
    if body.is_empty() {
        return true;
    }
    let Some((_, version)) = body.split_once("==") else {
        return false;
    };
    let version = version.trim();
    !version.is_empty() && !version.contains('*')
}

/// Whether a line of a requirements file is a requirement at all.
///
/// Comments, blank lines and pip's own options (`-r`, `-c`, `--index-url`,
/// `-e`) are not requirements this gate can judge.
pub fn is_requirement(line: &str) -> bool {
    let line = line.trim();
    !line.is_empty() && !line.starts_with('#') && !line.starts_with('-')
}

/// The requirements files in the tree, by the conventional names.
pub fn is_requirements_file(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.starts_with("requirements") && name.ends_with(".txt")
}

/// The `python-deps` gate: every requirement must pin its version.
pub fn dependencies(cfg: &Config) -> Result<i32, String> {
    let qcfg = crate::quality_config::QualityConfig::load(cfg.root())?.unwrap_or_default();
    let mut unpinned = Vec::new();
    let mut files = 0usize;
    let mut requirements = 0usize;
    for entry in fsutil::walk_files(cfg.root()) {
        if !is_requirements_file(&entry.rel) {
            continue;
        }
        if qcfg.should_ignore(&entry.rel, "deps") {
            continue;
        }
        let Some(data) = fsutil::read_file(&entry.path) else {
            continue;
        };
        files += 1;
        for (idx, raw) in String::from_utf8_lossy(&data).lines().enumerate() {
            if !is_requirement(raw) {
                continue;
            }
            requirements += 1;
            if !is_pinned(raw) {
                unpinned.push(Unpinned {
                    path: entry.rel.clone(),
                    line: idx + 1,
                    requirement: raw.trim().to_string(),
                });
            }
        }
    }

    let mut report = Report::new("python-deps");
    report.extra = Some(serde_json::json!({
        "files": files,
        "requirements": requirements,
    }));
    if files == 0 {
        report.summary = "python-deps: no requirements files, skipped".to_string();
        return Ok(crate::gates::emit(cfg, &GateResult::finish(report)));
    }
    for item in &unpinned {
        report.add(
            Diagnostic::error(
                item.path.clone(),
                "python.unpinned",
                format!("{} does not pin a version", item.requirement),
            )
            .at_line(item.line),
        );
    }
    if report.ok {
        report.summary =
            format!("python-deps: {requirements} requirement(s) in {files} file(s), all pinned");
        return Ok(crate::gates::emit(cfg, &GateResult::finish(report)));
    }
    report.summary = format!(
        "python-deps: {} of {requirements} requirement(s) are unpinned",
        unpinned.len()
    );
    let text = format!(
        "python-deps found {} requirement(s) without an exact version:\n{}",
        unpinned.len(),
        unpinned
            .iter()
            .map(|item| format!("  ✗ {}:{}: {}", item.path, item.line, item.requirement))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let result = GateResult::failed("python-deps", report.summary.clone(), report)
        .failing(text, format!("{} unpinned requirement(s)", unpinned.len()));
    Ok(crate::gates::emit(cfg, &result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_each_shell_call_once_per_line() {
        let text = "import os\nos.system('ls')\n# os.system('commented')\nsubprocess.run(cmd, shell=True)\n";
        let found = scan_source("a.py", text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].line, 2);
        assert_eq!(found[1].line, 4);
    }

    #[test]
    fn a_command_without_a_shell_is_not_a_finding() {
        assert!(scan_source("a.py", "subprocess.run(['ls', '-l'])\n").is_empty());
    }

    #[test]
    fn recognises_pins() {
        assert!(is_pinned("requests==2.31.0"));
        assert!(is_pinned("requests[security]==2.31.0"));
        assert!(is_pinned("requests==2.31.0; python_version > '3.8'"));
        assert!(!is_pinned("requests"));
        assert!(!is_pinned("requests>=2"));
        assert!(!is_pinned("requests~=2.31"));
        assert!(!is_pinned("requests==2.31.*"));
        assert!(!is_pinned("requests @ https://example.com/requests.whl"));
    }

    #[test]
    fn skips_comments_blanks_and_pip_options() {
        assert!(!is_requirement("# a comment"));
        assert!(!is_requirement("   "));
        assert!(!is_requirement("-r other.txt"));
        assert!(!is_requirement("--index-url https://example.com"));
        assert!(is_requirement("requests==2.31.0"));
    }

    #[test]
    fn recognises_requirements_files_by_convention() {
        assert!(is_requirements_file("requirements.txt"));
        assert!(is_requirements_file("requirements-dev.txt"));
        assert!(is_requirements_file("backend/requirements/prod.txt").eq(&false));
        assert!(!is_requirements_file("pyproject.toml"));
    }
}
