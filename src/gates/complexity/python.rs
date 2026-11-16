// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The Python and Mojo scanners for the `complexity` gate.
//!
//! Both languages mark a block with indentation instead of braces, so the
//! scanner is a different shape: a `def` header opens a body, and the body is
//! every following line indented deeper than the header (blank lines belong to
//! it). Decision points are counted with the keywords the other scanners use,
//! plus Python's own `elif`, `except`, `and` and `or`.
//!
//! Mojo is read with the same scanner: where this gate looks, a Mojo `def` and
//! a Python `def` are the same thing.

use super::lang::{count_keyword, FunctionMetric};

/// Scans a Python (or Mojo) source file.
pub fn scan_python(path: &str, source: &str, language: &str) -> Vec<FunctionMetric> {
    let cleaned = strip_python_noise(source);
    let lines: Vec<&str> = cleaned.lines().collect();
    let mut metrics = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((name, indent)) = code_def(lines[i], language == "mojo") else {
            i += 1;
            continue;
        };
        // The body runs until the indentation returns to the header's level.
        // A nested `def` is part of the outer body, which is the same rule the
        // brace scanners follow for a closure.
        let mut body = String::new();
        let mut j = i + 1;
        while j < lines.len() {
            let next = lines[j];
            if next.trim().is_empty() {
                body.push('\n');
                j += 1;
                continue;
            }
            if indent_of(next) <= indent {
                break;
            }
            body.push_str(next);
            body.push('\n');
            j += 1;
        }
        metrics.push(FunctionMetric {
            path: path.to_string(),
            line: i + 1,
            function: name,
            complexity: count_python_decisions(&body),
            language: language.to_string(),
        });
        i = j;
    }
    metrics
}

/// The leading-whitespace width of a line, with a tab counting as four.
fn indent_of(line: &str) -> usize {
    let mut width = 0;
    for ch in line.chars() {
        match ch {
            ' ' => width += 1,
            '\t' => width += 4,
            _ => break,
        }
    }
    width
}

/// The name and indentation of a function header, when `line` is one.
///
/// Python spells it `def` (and `async def`); Mojo 1.2 spells it `def` too —
/// `fn` is the spelling Mojo used to have, and is still accepted here so that a
/// source written for the older toolchain is still counted. Both open a block
/// with a colon and indentation, which is the only thing this scanner needs.
fn code_def(line: &str, allow_fn: bool) -> Option<(String, usize)> {
    let indent = indent_of(line);
    let rest = line.trim_start();
    let rest = rest.strip_prefix("async ").unwrap_or(rest);
    let rest = match rest.strip_prefix("def ") {
        Some(after) => after,
        None if allow_fn => rest.strip_prefix("fn ")?,
        None => return None,
    };
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        return None;
    }
    // A header ends its signature with a colon; anything else (a call, an
    // assignment) is not a definition.
    let after = &rest[name.len()..];
    after.contains(':').then_some((name, indent))
}

/// Counts the decision points in an indented body.
pub fn count_python_decisions(body: &str) -> usize {
    let mut complexity = 1;
    for keyword in ["if", "elif", "for", "while", "except", "case", "and", "or"] {
        complexity += count_keyword(body, keyword);
    }
    complexity
}

/// Removes Python's comments and string literals, keeping indentation.
///
/// The triple-quoted forms matter: a docstring in a module header is where
/// most of the prose in a Python file lives, and a false `if` inside it would
/// misreport a function's complexity.
pub fn strip_python_noise(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < chars.len() {
        i = match chars[i] {
            '#' => skip_comment(&chars, i),
            quote @ ('"' | '\'') => skip_string(&chars, quote, i, &mut out),
            ch => {
                out.push(ch);
                i + 1
            }
        };
    }
    out
}

/// Skips a comment, stopping on its newline so the line still ends.
fn skip_comment(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i] != '\n' {
        i += 1;
    }
    i
}

/// Skips the literal opening at `start`, writing the newlines it spans.
fn skip_string(chars: &[char], quote: char, start: usize, out: &mut String) -> usize {
    let end = if opens_triple(chars, quote, start) {
        skip_triple(chars, quote, start + 3, out)
    } else {
        skip_single(chars, quote, start + 1)
    };
    out.push(' ');
    end
}

/// Whether the literal opening at `start` is a triple-quoted one.
fn opens_triple(chars: &[char], quote: char, start: usize) -> bool {
    start + 2 < chars.len() && chars[start + 1] == quote && chars[start + 2] == quote
}

/// Whether the literal's closing three quotes begin at `i`.
fn closes_triple(chars: &[char], quote: char, i: usize) -> bool {
    i + 2 < chars.len() && chars[i] == quote && chars[i + 1] == quote && chars[i + 2] == quote
}

/// Skips a triple-quoted literal from `i`, keeping only its line breaks.
fn skip_triple(chars: &[char], quote: char, mut i: usize, out: &mut String) -> usize {
    while i < chars.len() && !closes_triple(chars, quote, i) {
        if chars[i] == '\n' {
            out.push('\n');
        }
        i += 1;
    }
    (i + 3).min(chars.len())
}

/// Skips a single-line literal from `i`, honouring backslash escapes.
///
/// An unterminated one stops at its own newline, which it leaves in place: the
/// reader after it has to see where the line ended.
fn skip_single(chars: &[char], quote: char, mut i: usize) -> usize {
    while i < chars.len() {
        match chars[i] {
            '\\' => i = (i + 2).min(chars.len()),
            c if c == quote => return i + 1,
            '\n' => return i,
            _ => i += 1,
        }
    }
    i
}
