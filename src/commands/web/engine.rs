// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The dependency-free scanner behind `web-lint`.
//!
//! Go's engine is a full HTML parser plus esbuild's JavaScript parser. This
//! rewrite keeps the part that catches the mistakes a merge introduces: the
//! tag structure of HTML and the bracket structure of CSS and scripts, with
//! the position of the first problem reported the way Go reports its own.

use std::path::Path;

/// One lint finding, positioned inside its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// One-based line.
    pub line: usize,
    /// One-based column.
    pub col: usize,
    /// `error`, `warning` or `info`.
    pub severity: &'static str,
    /// The rule that was broken.
    pub rule: String,
    /// A one-line description.
    pub message: String,
}

impl Finding {
    /// A finding at a position.
    fn error(line: usize, col: usize, rule: impl Into<String>, message: String) -> Self {
        Self {
            line,
            col,
            severity: "error",
            rule: rule.into(),
            message,
        }
    }
}

/// The language a path is linted as, or `""` when it is not a web source.
pub fn language_of(path: &Path) -> &'static str {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return "";
    };
    match ext.to_ascii_lowercase().as_str() {
        "html" | "htm" => "html",
        "css" => "css",
        "js" | "mjs" | "cjs" => "js",
        "jsx" => "jsx",
        "ts" | "mts" | "cts" => "ts",
        "tsx" => "tsx",
        _ => "",
    }
}

/// The engine label Go attaches to a language.
pub fn engine_of(language: &str) -> &'static str {
    match language {
        "html" => "builtin-html",
        "css" => "builtin-css",
        "js" => "builtin-js",
        "jsx" => "builtin-jsx",
        "ts" | "tsx" => "builtin-ts",
        _ => "-",
    }
}

/// The file name an inline snippet is reported under.
pub fn inline_name(language: &str) -> String {
    let ext = match language {
        "html" | "css" | "js" | "ts" | "tsx" | "jsx" => language,
        _ => "txt",
    };
    format!("inline.{ext}")
}

/// Lints one source, returning the findings in source order.
pub fn lint(language: &str, source: &str) -> Vec<Finding> {
    let finding = match language {
        "html" => check_html_tags(source),
        "css" | "js" | "jsx" | "ts" | "tsx" => check_brackets(language, source),
        _ => None,
    };
    finding.into_iter().collect()
}

/// The line and column of a byte offset.
pub fn position(source: &str, offset: usize) -> (usize, usize) {
    let limit = offset.min(source.len());
    let mut line = 1usize;
    let mut col = 1usize;
    for ch in source[..limit].chars() {
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Checks the bracket structure of a CSS or script source.
///
/// A bracket still open at the end of the file is the truncated-source
/// mistake a merge makes most often, so it is reported at the end of the
/// file with the message Go's parser uses.
pub fn check_brackets(language: &str, source: &str) -> Option<Finding> {
    let mut stack: Vec<(char, usize, usize)> = Vec::new();
    let mut chars = source.char_indices().peekable();
    while let Some((offset, ch)) = chars.next() {
        match ch {
            '/' if chars.peek().map(|(_, c)| *c) == Some('/') => skip_line_comment(&mut chars),
            '/' if chars.peek().map(|(_, c)| *c) == Some('*') => skip_block_comment(&mut chars),
            '"' | '\'' | '`' => skip_string(&mut chars, ch),
            '{' | '[' | '(' => {
                let (line, col) = position(source, offset);
                stack.push((ch, line, col));
            }
            '}' | ']' | ')' => {
                let (line, col) = position(source, offset);
                if let Some(finding) = closing(language, &mut stack, ch, line, col) {
                    return Some(finding);
                }
            }
            _ => {}
        }
    }
    stack.pop().map(|_| {
        let (line, col) = position(source, source.len());
        Finding::error(
            line,
            col,
            format!("{language}/syntax"),
            "Unexpected end of file".to_string(),
        )
    })
}

/// Consumes the rest of a `//` comment.
fn skip_line_comment(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>) {
    for (_, next) in chars.by_ref() {
        if next == '\n' {
            break;
        }
    }
}

/// Consumes a `/* … */` comment.
fn skip_block_comment(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>) {
    // The opening `*` has already been seen, so it is the first closer to be.
    let mut previous = '*';
    for (_, next) in chars.by_ref() {
        if previous == '*' && next == '/' {
            break;
        }
        previous = next;
    }
}

/// Skips over a string literal.
fn skip_string(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>, quote: char) {
    let mut previous = '\0';
    for (_, ch) in chars.by_ref() {
        if previous == '\\' {
            previous = '\0';
            continue;
        }
        if ch == quote {
            return;
        }
        previous = ch;
    }
}

/// Applies a closing bracket to the stack, reporting the first mismatch.
fn closing(
    language: &str,
    stack: &mut Vec<(char, usize, usize)>,
    ch: char,
    line: usize,
    col: usize,
) -> Option<Finding> {
    let expected = match ch {
        '}' => '{',
        ']' => '[',
        _ => '(',
    };
    match stack.pop() {
        Some((open, _, _)) if open == expected => None,
        _ => Some(Finding::error(
            line,
            col,
            format!("{language}/syntax"),
            format!("Unexpected \"{ch}\""),
        )),
    }
}

/// Checks the tag structure of an HTML document.
pub fn check_html_tags(source: &str) -> Option<Finding> {
    let mut stack: Vec<(String, usize, usize)> = Vec::new();
    let mut cursor = 0usize;
    while let Some(start) = source[cursor..].find('<') {
        let tag_start = cursor + start;
        let after = &source[tag_start + 1..];
        let (line, col) = position(source, tag_start);
        let Some(end) = after.find('>') else {
            return Some(Finding::error(
                line,
                col,
                "html/unclosed-tag",
                "a tag is never closed".to_string(),
            ));
        };
        let tag = after[..end].trim();
        cursor = tag_start + 1 + end + 1;
        if tag.starts_with('!') || tag.starts_with('?') {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            let name = name.trim().to_lowercase();
            match stack.pop() {
                Some((open, _, _)) if open == name => {}
                Some((open, _, _)) => {
                    return Some(Finding::error(
                        line,
                        col,
                        "html/mismatched-tag",
                        format!("</{name}> closes <{open}>"),
                    ))
                }
                None => {
                    return Some(Finding::error(
                        line,
                        col,
                        "html/mismatched-tag",
                        format!("</{name}> has no opening tag"),
                    ))
                }
            }
            continue;
        }
        if tag.ends_with('/') {
            continue;
        }
        let name = tag.split_whitespace().next().unwrap_or("").to_lowercase();
        if !name.is_empty() && !VOID_ELEMENTS.contains(&name.as_str()) {
            stack.push((name, line, col));
        }
    }
    stack.pop().map(|(open, line, col)| {
        Finding::error(
            line,
            col,
            "html/unclosed-tag",
            format!("<{open}> is never closed"),
        )
    })
}

/// The HTML elements that never have a closing tag.
const VOID_ELEMENTS: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

#[cfg(test)]
mod tests;
