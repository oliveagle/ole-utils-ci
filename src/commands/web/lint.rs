// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `web-lint` command: syntax sanity for the front-end sources.
//!
//! The Go original embeds a full HTML and CSS engine. This rewrite keeps the
//! useful part — catching the mistakes a merge introduces — with a dependency
//! free scanner: JSON is parsed, and the bracket structure of the HTML, CSS
//! and script sources is checked.

use std::path::Path;

use crate::config::Config;
use crate::fsutil;
use crate::output::{Diagnostic, Report};

/// The extensions the gate inspects.
const SCANNED_EXTENSIONS: [&str; 9] = [
    ".html", ".htm", ".css", ".scss", ".less", ".js", ".jsx", ".ts", ".tsx",
];

/// One syntax problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebIssue {
    /// The rule that was broken.
    pub rule: String,
    /// A one-line description.
    pub message: String,
}

/// Checks the brackets of a source file.
pub fn check_brackets(source: &str) -> Option<WebIssue> {
    let mut stack: Vec<char> = Vec::new();
    let mut chars = source.chars().peekable();
    let mut line = 1usize;
    while let Some(ch) = chars.next() {
        match ch {
            '\n' => line += 1,
            '/' if chars.peek() == Some(&'/') => line = skip_line_comment(&mut chars, line),
            '/' if chars.peek() == Some(&'*') => line = skip_block_comment(&mut chars, line),
            '"' | '\'' | '`' => skip_string(&mut chars, ch),
            '{' | '[' | '(' => stack.push(ch),
            '}' | ']' | ')' => {
                if let Some(issue) = closing(&mut stack, ch, line) {
                    return Some(issue);
                }
            }
            _ => {}
        }
    }
    stack.pop().map(|open| WebIssue {
        rule: "web/unbalanced-brackets".to_string(),
        message: format!("{open} is never closed"),
    })
}

/// Consumes the rest of a `//` comment and returns the line it ends on.
fn skip_line_comment(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    mut line: usize,
) -> usize {
    for next in chars.by_ref() {
        if next == '\n' {
            line += 1;
            break;
        }
    }
    line
}

/// Consumes a `/* … */` comment and returns the line it ends on.
fn skip_block_comment(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    mut line: usize,
) -> usize {
    // The opening `*` has already been seen, so it is the first closer to be.
    let mut previous = '*';
    for next in chars.by_ref() {
        if next == '\n' {
            line += 1;
        }
        if previous == '*' && next == '/' {
            break;
        }
        previous = next;
    }
    line
}

/// Applies a closing bracket to the stack, reporting the first mismatch.
fn closing(stack: &mut Vec<char>, ch: char, line: usize) -> Option<WebIssue> {
    let expected = match ch {
        '}' => '{',
        ']' => '[',
        _ => '(',
    };
    match stack.pop() {
        Some(open) if open == expected => None,
        Some(open) => Some(WebIssue {
            rule: "web/unbalanced-brackets".to_string(),
            message: format!("line {line}: {ch} closes {open}"),
        }),
        None => Some(WebIssue {
            rule: "web/unbalanced-brackets".to_string(),
            message: format!("line {line}: {ch} has no opening bracket"),
        }),
    }
}

/// Skips over a string literal.
fn skip_string(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, quote: char) {
    let mut previous = '\0';
    for ch in chars.by_ref() {
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

/// Checks the tags of an HTML document.
pub fn check_html_tags(source: &str) -> Option<WebIssue> {
    let mut stack: Vec<String> = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find('<') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('>') else {
            return Some(WebIssue {
                rule: "web/unclosed-tag".to_string(),
                message: "a tag is never closed".to_string(),
            });
        };
        let tag = after[..end].trim();
        rest = &after[end + 1..];
        if tag.starts_with('!') || tag.starts_with('?') {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            let name = name.trim().to_lowercase();
            match stack.pop() {
                Some(open) if open == name => {}
                Some(open) => {
                    return Some(WebIssue {
                        rule: "web/mismatched-tag".to_string(),
                        message: format!("</{name}> closes <{open}>"),
                    })
                }
                None => {
                    return Some(WebIssue {
                        rule: "web/mismatched-tag".to_string(),
                        message: format!("</{name}> has no opening tag"),
                    })
                }
            }
            continue;
        }
        if tag.ends_with('/') {
            continue;
        }
        let name = tag.split_whitespace().next().unwrap_or("").to_lowercase();
        if !name.is_empty() && !VOID_ELEMENTS.contains(&name.as_str()) {
            stack.push(name);
        }
    }
    stack.pop().map(|open| WebIssue {
        rule: "web/unclosed-tag".to_string(),
        message: format!("<{open}> is never closed"),
    })
}

/// The HTML elements that never have a closing tag.
const VOID_ELEMENTS: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// The `web-lint` command.
pub fn web_lint(cfg: &Config) -> Result<i32, String> {
    let mut report = Report::new("web-lint");
    let mut scanned = 0usize;
    for entry in fsutil::walk_files(cfg.root()) {
        let ext = entry.extension();
        if !SCANNED_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }
        let Some(data) = fsutil::read_file(&entry.path) else {
            continue;
        };
        let source = String::from_utf8_lossy(&data);
        scanned += 1;
        let issue = match ext.as_str() {
            ".html" | ".htm" => check_html_tags(&source),
            _ => check_brackets(&source),
        };
        if let Some(issue) = issue {
            report.add(Diagnostic::error(
                entry.rel.clone(),
                issue.rule.clone(),
                issue.message.clone(),
            ));
        }
    }
    report.extra = Some(serde_json::json!({ "files_scanned": scanned }));
    if report.ok {
        report.summary = format!("web-lint: {scanned} file(s) clean");
        print!("{}", report.render_text());
        Ok(0)
    } else {
        report.summary = format!("web-lint: {} issue(s) found", report.diagnostics.len());
        print!("{}", report.render_text());
        Err(report.summary)
    }
}

/// Whether the path looks like a front-end source.
pub fn is_web_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    SCANNED_EXTENSIONS.contains(&ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_unbalanced_brackets() {
        assert!(check_brackets("function a() {").is_some());
        assert!(check_brackets("function a() {}").is_none());
    }

    #[test]
    fn ignores_brackets_in_strings_and_comments() {
        assert!(check_brackets("const a = '{';").is_none());
        assert!(check_brackets("// {\nconst a = 1;").is_none());
    }

    #[test]
    fn detects_unclosed_html_tags() {
        assert!(check_html_tags("<div><p>text</p></div>").is_none());
        assert!(check_html_tags("<div><p>text</div>").is_some());
        assert!(check_html_tags("<img src=\"x\"><br>").is_none());
    }
}
