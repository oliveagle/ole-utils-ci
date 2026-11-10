// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Function extraction for the languages the complexity gate understands.

use super::lang::{count_decisions, match_body, strip_noise, FunctionMetric};

/// Scans a Rust source file.
pub fn scan_rust(path: &str, source: &str) -> Vec<FunctionMetric> {
    let cleaned = strip_noise(source);
    let chars: Vec<char> = cleaned.chars().collect();
    let mut metrics = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !at_word_start(&chars, i, "fn") {
            i += 1;
            continue;
        }
        let Some((name, body_start)) = rust_signature(&chars, i) else {
            i += 1;
            continue;
        };
        let Some((body, end)) = match_body(&chars, body_start) else {
            i += 1;
            continue;
        };
        metrics.push(metric(path, &cleaned, i, &name, &body, "rust"));
        i = end;
    }
    metrics
}

/// Finds the name of a Rust function and where its body starts.
fn rust_signature(chars: &[char], fn_index: usize) -> Option<(String, usize)> {
    let mut i = fn_index + 2;
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    let start = i;
    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
        i += 1;
    }
    if start == i {
        return None;
    }
    let name: String = chars[start..i].iter().collect();
    let body_start = find_next_brace(chars, i)?;
    Some((name, body_start))
}

/// Finds the index of the next `{` at or after `from`, skipping over
/// generic parameter lists and return types.
fn find_next_brace(chars: &[char], from: usize) -> Option<usize> {
    let mut i = from;
    let mut angle_depth = 0i32;
    while i < chars.len() {
        match chars[i] {
            '<' => angle_depth += 1,
            '>' => angle_depth -= 1,
            '{' if angle_depth <= 0 => return Some(i),
            ';' if angle_depth <= 0 => return None,
            _ => {}
        }
        i += 1;
    }
    None
}

/// Scans a Go source file.
pub fn scan_go(path: &str, source: &str) -> Vec<FunctionMetric> {
    let cleaned = strip_noise(source);
    let chars: Vec<char> = cleaned.chars().collect();
    let mut metrics = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !at_word_start(&chars, i, "func") {
            i += 1;
            continue;
        }
        let Some((name, body_start)) = go_signature(&chars, i) else {
            i += 1;
            continue;
        };
        let Some((body, end)) = match_body(&chars, body_start) else {
            i += 1;
            continue;
        };
        metrics.push(metric(path, &cleaned, i, &name, &body, "go"));
        i = end;
    }
    metrics
}

/// Finds the name of a Go function and where its body starts.
fn go_signature(chars: &[char], func_index: usize) -> Option<(String, usize)> {
    let mut i = func_index + 4;
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    if i < chars.len() && chars[i] == '(' {
        let close = find_matching_paren(chars, i)?;
        i = close + 1;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
    }
    let start = i;
    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
        i += 1;
    }
    if start == i {
        return None;
    }
    let name: String = chars[start..i].iter().collect();
    let body_start = find_next_brace(chars, i)?;
    Some((name, body_start))
}

fn find_matching_paren(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open;
    while i < chars.len() {
        match chars[i] {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Scans a TypeScript, JavaScript or Tengo source file.
pub fn scan_script(path: &str, source: &str, language: &str) -> Vec<FunctionMetric> {
    let cleaned = strip_noise(source);
    let chars: Vec<char> = cleaned.chars().collect();
    let mut metrics = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if at_word_start(&chars, i, "function") {
            if let Some(found) = script_function(path, &chars, &cleaned, i, language) {
                metrics.push(found.metric);
                i = found.next;
                continue;
            }
        }
        if let Some(found) = arrow_function(path, &chars, &cleaned, i, language) {
            metrics.push(found.metric);
            i = found.next;
            continue;
        }
        i += 1;
    }
    metrics
}

struct Found {
    metric: FunctionMetric,
    next: usize,
}

fn script_function(
    path: &str,
    chars: &[char],
    cleaned: &str,
    index: usize,
    language: &str,
) -> Option<Found> {
    let mut i = index + 8;
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    let start = i;
    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$') {
        i += 1;
    }
    if start == i {
        return None;
    }
    let name: String = chars[start..i].iter().collect();
    let body_start = find_next_brace(chars, i)?;
    let (body, end) = match_body(chars, body_start)?;
    Some(Found {
        metric: metric(path, cleaned, index, &name, &body, language),
        next: end,
    })
}

fn arrow_function(
    path: &str,
    chars: &[char],
    cleaned: &str,
    index: usize,
    language: &str,
) -> Option<Found> {
    let mut i = index;
    while i < chars.len() && (chars[i].is_whitespace() || chars[i] == '=') {
        i += 1;
    }
    if i >= chars.len() || chars[i] != '(' {
        return None;
    }
    let paren_end = find_matching_paren(chars, i)?;
    let mut j = paren_end + 1;
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    if j >= chars.len() || chars[j] != '{' {
        return None;
    }
    let name = enclosing_name(chars, index);
    let (body, end) = match_body(chars, j)?;
    Some(Found {
        metric: metric(path, cleaned, index, &name, &body, language),
        next: end,
    })
}

/// The identifier an arrow function is bound to, e.g. `const add = …`.
fn enclosing_name(chars: &[char], index: usize) -> String {
    let mut i = index;
    while i > 0 && chars[i] != '=' {
        i -= 1;
    }
    let mut end = i;
    while end > 0 && chars[end - 1].is_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }
    if start == end {
        return "<anonymous>".to_string();
    }
    chars[start..end].iter().collect()
}

/// Whether `word` starts at `index` and is not part of a longer identifier.
fn at_word_start(chars: &[char], index: usize, word: &str) -> bool {
    let needle: Vec<char> = word.chars().collect();
    if index + needle.len() > chars.len() {
        return false;
    }
    if !chars[index..index + needle.len()].starts_with(&needle[..]) {
        return false;
    }
    if index > 0 {
        let prev = chars[index - 1];
        if prev.is_alphanumeric() || prev == '_' {
            return false;
        }
    }
    true
}

fn metric(
    path: &str,
    cleaned: &str,
    index: usize,
    name: &str,
    body: &str,
    language: &str,
) -> FunctionMetric {
    FunctionMetric {
        path: path.to_string(),
        line: cleaned[..index].chars().filter(|c| *c == '\n').count() + 1,
        function: name.to_string(),
        complexity: count_decisions(body),
        language: language.to_string(),
    }
}
