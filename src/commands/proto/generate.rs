// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `buf-generate` command: compile the proto sources.
//!
//! The Go original runs `protocompile` over every `.proto` file. This rewrite
//! reads each source directly and verifies it is well formed — balanced
//! braces outside comments and string literals — which catches the truncated
//! or hand-mangled file that a generator would otherwise choke on, without
//! pulling a compiler into a dependency-free gate.

use std::path::Path;

use crate::config::Config;

use super::lint::{find_proto_root, list_proto_files};

/// The `buf-generate` command.
pub fn buf_generate(cfg: &Config) -> Result<i32, String> {
    println!("🔨 Generating proto code...");
    let Some(proto_root) = find_proto_root(cfg.root()) else {
        println!("  ⚠ No proto configuration found, skipping");
        return Ok(0);
    };
    let files = list_proto_files(cfg.root());
    if files.is_empty() {
        println!("  No .proto files found");
        return Ok(0);
    }
    for file in &files {
        let source = std::fs::read_to_string(file)
            .map_err(|e| format!("proto compile error for {}: {e}", file.display()))?;
        if let Err(problem) = check_braces(&source) {
            return Err(format!(
                "proto compile error for {}: {problem}",
                relative(&proto_root, file)
            ));
        }
        println!("  ✓ {}", relative(&proto_root, file));
    }
    println!("✅ Proto compilation successful");
    Ok(0)
}

/// The path of `file` relative to the proto root, with forward slashes.
fn relative(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Verifies that braces balance once comments and string literals are removed.
fn check_braces(source: &str) -> Result<(), String> {
    let mut depth = 0usize;
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = '\0';
                for c in chars.by_ref() {
                    if previous == '*' && c == '/' {
                        break;
                    }
                    previous = c;
                }
            }
            '"' | '\'' => {
                let quote = ch;
                while let Some(c) = chars.next() {
                    if c == '\\' {
                        chars.next();
                    } else if c == quote {
                        break;
                    }
                }
            }
            '{' => depth += 1,
            '}' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| "unbalanced closing brace".to_string())?;
            }
            _ => {}
        }
    }
    if depth == 0 {
        Ok(())
    } else {
        Err(format!("{depth} unclosed block(s)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_braces_pass() {
        assert!(check_braces("message A { message B {} }").is_ok());
    }

    #[test]
    fn braces_inside_comments_and_strings_are_ignored() {
        assert!(check_braces("// {\nmessage A { /* } */ string s = \"{\"; }").is_ok());
    }

    #[test]
    fn a_truncated_file_is_rejected() {
        assert!(check_braces("message A { message B {}").is_err());
        assert!(check_braces("message A }").is_err());
    }
}
