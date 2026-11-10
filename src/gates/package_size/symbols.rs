// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Counting the exported top-level symbols of a source file.

/// Counts exported top-level symbols.
pub fn count_exported_symbols(source: &str, ext: &str) -> usize {
    if ext == ".rs" {
        rust_exported_symbols(source)
    } else {
        go_exported_symbols(source)
    }
}

/// Counts the exported declarations of a Go file.
fn go_exported_symbols(source: &str) -> usize {
    let mut count = 0;
    for line in source.lines() {
        let trimmed = line.trim_start();
        let rest = if let Some(r) = trimmed.strip_prefix("func ") {
            r
        } else if let Some(r) = trimmed.strip_prefix("type ") {
            r
        } else {
            continue;
        };
        let symbol = go_symbol_name(rest);
        if symbol.chars().next().is_some_and(|c| c.is_uppercase()) {
            count += 1;
        }
    }
    count
}

/// The exported name of a Go declaration, ignoring any receiver.
fn go_symbol_name(rest: &str) -> String {
    let rest = rest.trim_start();
    let rest = if rest.starts_with('(') {
        match rest.find(')') {
            Some(idx) => rest[idx + 1..].trim_start(),
            None => rest,
        }
    } else {
        rest
    };
    rest.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Counts the `pub` items of a Rust file that are visible outside the crate.
fn rust_exported_symbols(source: &str) -> usize {
    let mut count = 0;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("pub ") {
            continue;
        }
        let rest = trimmed.trim_start_matches("pub ").trim_start();
        let rest = rest
            .strip_prefix("crate ")
            .or_else(|| rest.strip_prefix("super "))
            .or_else(|| rest.strip_prefix("self "))
            .unwrap_or(rest)
            .trim_start();
        if rest.starts_with('(') {
            continue;
        }
        if is_rust_item_keyword(rest) {
            count += 1;
        }
    }
    count
}

/// Whether the declaration starts with a Rust item keyword.
fn is_rust_item_keyword(rest: &str) -> bool {
    const KEYWORDS: [&str; 7] = [
        "fn ", "struct ", "enum ", "trait ", "const ", "static ", "type ",
    ];
    KEYWORDS.iter().any(|k| rest.starts_with(k))
}
