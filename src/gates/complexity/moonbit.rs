// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The MoonBit scanner: `fn` headers with brace-delimited bodies.

use super::lang::{match_body, strip_noise, FunctionMetric};
use super::scanners::{at_word_start, find_next_brace};

pub fn scan_moonbit(path: &str, source: &str) -> Vec<FunctionMetric> {
    let cleaned = strip_noise(source);
    let chars: Vec<char> = cleaned.chars().collect();
    let mut metrics = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !at_word_start(&chars, i, "fn") {
            i += 1;
            continue;
        }
        let Some((name, body_start)) = moonbit_signature(&chars, i) else {
            i += 1;
            continue;
        };
        let Some((body, end)) = match_body(&chars, body_start) else {
            i += 1;
            continue;
        };
        metrics.push(FunctionMetric {
            path: path.to_string(),
            line: cleaned[..i].chars().filter(|c| *c == '\n').count() + 1,
            function: name,
            complexity: count_moonbit_decisions(&body),
            language: "moonbit".to_string(),
        });
        i = end;
    }
    metrics
}

/// Finds a MoonBit function name and its body start.
fn moonbit_signature(chars: &[char], fn_index: usize) -> Option<(String, usize)> {
    let mut i = fn_index + 2;
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    let start = i;
    while i < chars.len()
        && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == ':' || chars[i] == '$')
    {
        i += 1;
    }
    if start == i {
        return None;
    }
    let name: String = chars[start..i].iter().collect();
    let body_start = find_next_brace(chars, i)?;
    Some((name, body_start))
}

/// Counts MoonBit decisions: guards, loops, match arms and boolean operators.
fn count_moonbit_decisions(body: &str) -> usize {
    let mut complexity = 1;
    for keyword in ["if", "for", "while", "guard", "catch"] {
        complexity += super::lang::count_keyword(body, keyword);
    }
    complexity += body.matches("=>").count();
    complexity += body.matches("&&").count();
    complexity += body.matches("||").count();
    complexity
}
