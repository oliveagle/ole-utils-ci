// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Per-language function extraction and cyclomatic-complexity counting.
//!
//! The Go original parses Go with `go/ast` and counts `if`, `for`, `range`,
//! `case`, `select` clauses and `&&`/`||`. Rust has no equivalent parser in
//! this crate, so the scanners below are brace-aware text scanners: they find
//! a function header, match its body with a brace counter, and count decision
//! points in the body with comments and string literals removed.
//!
//! The counting rules mirror the Go ones so a project that mixes languages
//! sees one consistent budget.

/// The languages the Go original reports, in its order.
pub const LANGUAGES: [&str; 4] = ["go", "tengo", "ts", "js"];

/// The extra scanner this port adds for Rust projects.
pub const RUST_LANGUAGE: &str = "rust";

/// One function found in a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionMetric {
    /// Path relative to the project root.
    pub path: String,
    /// 1-based line the function header starts on.
    pub line: usize,
    /// The function's name.
    pub function: String,
    /// Its cyclomatic complexity.
    pub complexity: usize,
    /// The language it was written in.
    pub language: String,
}

/// The language of a file, by extension.
pub fn language_for_extension(ext: &str) -> Option<&'static str> {
    Some(match ext {
        ".rs" => "rust",
        ".go" => "go",
        ".tengo" => "tengo",
        ".ts" | ".tsx" | ".mts" | ".cts" => "ts",
        ".js" | ".jsx" | ".mjs" | ".cjs" => "js",
        _ => return None,
    })
}

/// Scans one source file and returns the complexity of each function in it.
pub fn scan_file(path: &str, source: &str, language: &str) -> Vec<FunctionMetric> {
    use super::scanners::{scan_go, scan_rust, scan_script};
    match language {
        "rust" => scan_rust(path, source),
        "go" => scan_go(path, source),
        "ts" | "js" => scan_script(path, source, language),
        "tengo" => scan_script(path, source, "tengo"),
        _ => Vec::new(),
    }
}

/// Removes comments and string literals so keyword counting is not fooled by
/// prose.
pub fn strip_noise(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '/' if i + 1 < chars.len() && chars[i + 1] == '/' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(chars.len());
            }
            '"' | '\'' | '`' => {
                let quote = ch;
                i += 1;
                while i < chars.len() {
                    if chars[i] == '\\' {
                        i += 2;
                        continue;
                    }
                    if chars[i] == quote {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                out.push(' ');
            }
            _ => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out
}

/// Counts decision points in a function body.
///
/// The rules match the Go original: each `if`, each loop, each non-default
/// `case`/`match` arm, and each `&&`/`||` adds one.
pub fn count_decisions(body: &str) -> usize {
    let mut complexity = 1;
    for (word, count) in decision_keywords(body) {
        let _ = word;
        complexity += count;
    }
    complexity
}

fn decision_keywords(body: &str) -> Vec<(&'static str, usize)> {
    let mut out = Vec::new();
    for keyword in ["if", "for", "while", "case"] {
        out.push((keyword, count_keyword(body, keyword)));
    }
    out.push(("&&", body.matches("&&").count()));
    out.push(("||", body.matches("||").count()));
    out
}

/// Counts standalone occurrences of a keyword.
pub fn count_keyword(body: &str, keyword: &str) -> usize {
    let mut count = 0;
    let bytes: Vec<char> = body.chars().collect();
    let needle: Vec<char> = keyword.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(&needle[..]) {
            let before_ok = i == 0 || !is_word_char(bytes[i - 1]);
            let after = i + needle.len();
            let after_ok = after >= bytes.len() || !is_word_char(bytes[after]);
            if before_ok && after_ok {
                count += 1;
            }
            i = after;
            continue;
        }
        i += 1;
    }
    count
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Finds the body of a function that starts at `open_brace`, matching braces.
///
/// Returns the body text and the index just past the closing brace.
pub fn match_body(chars: &[char], open_brace: usize) -> Option<(String, usize)> {
    if open_brace >= chars.len() || chars[open_brace] != '{' {
        return None;
    }
    let mut depth = 0usize;
    let mut i = open_brace;
    while i < chars.len() {
        match chars[i] {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((chars[open_brace..i].iter().collect(), i + 1));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}
