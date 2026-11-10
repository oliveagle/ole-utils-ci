// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Collecting the imports of a source file.

/// Collects the imports of a source file.
pub fn collect_imports(source: &str, ext: &str) -> Vec<String> {
    if ext == ".rs" {
        rust_imports(source)
    } else {
        go_imports(source)
    }
}

/// Collects the import paths of a Go file.
fn go_imports(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_block = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if in_block {
            if trimmed == ")" {
                in_block = false;
                continue;
            }
            out.push(trimmed.trim_matches('"').to_string());
            continue;
        }
        if trimmed.starts_with("import (") {
            in_block = true;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("import ") {
            out.push(rest.trim().trim_matches('"').to_string());
        }
    }
    out
}

/// Collects the `use` paths of a Rust file.
fn rust_imports(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        let rest = if let Some(r) = trimmed.strip_prefix("use ") {
            r
        } else if let Some(r) = trimmed.strip_prefix("pub use ") {
            r
        } else {
            continue;
        };
        out.push(rest.trim().to_string());
    }
    out
}
