// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `mojo-unsafe` gate: no `unsafe_*` call in production sources.
//!
//! Mojo's `unsafe_*` family (`unsafe_ptr`, `unsafe_load`, `unsafe_store`,
//! `unsafe_bitcast`, …) is the language's escape hatch from the alias and
//! lifetime checking that makes Mojo worth writing. The gate is the Mojo
//! counterpart of `rust-unsafe`: the same argument, the same shape, the same
//! answer — an escape hatch is a decision, so it is allowed only where the
//! project explicitly says so.
//!
//! The scan reads text, not a compiler's view: a hit inside a docstring is
//! still a hit. That is the same trade `no-shell-exec` makes, and it is the
//! right one for a gate that has to run without the toolchain.

use crate::config::Config;
use crate::fsutil;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};

/// The prefix every escape hatch in the Mojo standard library carries.
pub const UNSAFE_PREFIX: &str = "unsafe_";

/// One `unsafe_*` use found in the sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsafeUse {
    /// Path relative to the project root.
    pub path: String,
    /// 1-based line number.
    pub line: usize,
    /// The identifier that was used.
    pub identifier: String,
}

/// The code part of a line: a `#` comment is dropped.
fn code_part(line: &str) -> &str {
    match line.find('#') {
        Some(idx) => &line[..idx],
        None => line,
    }
}

/// Whether `ch` can be part of an identifier.
fn is_ident(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// The `unsafe_*` identifiers on one line, in the order they appear.
pub fn identifiers_in(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let needle: Vec<char> = UNSAFE_PREFIX.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i..].starts_with(&needle[..]) {
            i += 1;
            continue;
        }
        // The prefix must start an identifier, not end one: `my_unsafe_ptr` is
        // a different name and would be somebody's own wrapper.
        let starts_word = i == 0 || !is_ident(chars[i - 1]);
        if !starts_word {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i + needle.len();
        if end >= chars.len() || !is_ident(chars[end]) {
            // A bare `unsafe_` is not a call; nothing to report.
            i = end;
            continue;
        }
        while end < chars.len() && is_ident(chars[end]) {
            end += 1;
        }
        found.push(chars[start..end].iter().collect());
        i = end;
    }
    found
}

/// Scans one source file's text for `unsafe_*` uses.
pub fn scan_source(rel: &str, text: &str) -> Vec<UnsafeUse> {
    let mut found = Vec::new();
    for (idx, raw) in text.lines().enumerate() {
        for identifier in identifiers_in(code_part(raw)) {
            found.push(UnsafeUse {
                path: rel.to_string(),
                line: idx + 1,
                identifier,
            });
        }
    }
    found
}

/// The `mojo-unsafe` gate.
pub fn no_unsafe(cfg: &Config) -> Result<i32, String> {
    let qcfg = crate::quality_config::QualityConfig::load(cfg.root())?.unwrap_or_default();
    let mut findings = Vec::new();
    let mut scanned = 0usize;
    for entry in fsutil::walk_files(cfg.root()) {
        if !super::SOURCE_EXTENSIONS.contains(&entry.extension().as_str()) {
            continue;
        }
        if qcfg.should_ignore(&entry.rel, "unsafe") {
            continue;
        }
        scanned += 1;
        let Some(data) = fsutil::read_file(&entry.path) else {
            continue;
        };
        findings.extend(scan_source(&entry.rel, &String::from_utf8_lossy(&data)));
    }

    let mut report = Report::new("mojo-unsafe");
    report.extra = Some(serde_json::json!({
        "files_scanned": scanned,
        "prefix": UNSAFE_PREFIX,
    }));
    for finding in &findings {
        report.add(
            Diagnostic::error(
                finding.path.clone(),
                "mojo.unsafe",
                format!(
                    "{} is one of Mojo's safety escape hatches",
                    finding.identifier
                ),
            )
            .at_line(finding.line),
        );
    }
    if report.ok {
        report.summary = format!("mojo-unsafe: {scanned} file(s), no unsafe_* call");
        return Ok(crate::gates::emit(cfg, &GateResult::finish(report)));
    }
    report.summary = format!(
        "mojo-unsafe: {} unsafe_* use(s) in {scanned} file(s)",
        findings.len()
    );
    let text = format!(
        "mojo-unsafe found {} use(s) of a safety escape hatch:\n{}",
        findings.len(),
        findings
            .iter()
            .map(|f| format!("  ✗ {}:{}: {}", f.path, f.line, f.identifier))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let result = GateResult::failed("mojo-unsafe", report.summary.clone(), report)
        .failing(text, format!("{} unsafe_* use(s)", findings.len()));
    Ok(crate::gates::emit(cfg, &result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_escape_hatches() {
        let text = "let p = unsafe_ptr[x](v)\nlet x = unsafe_load(p)\n";
        let found = scan_source("a.mojo", text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].identifier, "unsafe_ptr");
        assert_eq!(found[1].identifier, "unsafe_load");
        assert_eq!(found[1].line, 2);
    }

    #[test]
    fn ignores_comments() {
        assert!(scan_source("a.mojo", "# unsafe_ptr is not used here\n").is_empty());
        assert!(scan_source("a.mojo", "let y = 1  # unsafe_store(x)\n").is_empty());
    }

    #[test]
    fn does_not_match_a_longer_identifier() {
        assert!(scan_source("a.mojo", "let z = my_unsafe_ptr(x)\n").is_empty());
    }

    #[test]
    fn a_bare_prefix_is_not_a_call() {
        assert!(scan_source("a.mojo", "let name = unsafe_\n").is_empty());
    }

    #[test]
    fn reports_every_use_on_one_line() {
        let found = scan_source("a.mojo", "x = unsafe_load(unsafe_ptr[y](v))\n");
        assert_eq!(found.len(), 2);
    }
}
