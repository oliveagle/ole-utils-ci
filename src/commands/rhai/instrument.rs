// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Branch-site discovery and source instrumentation.

use super::model::{BranchSite, BRANCH_PROBE};

pub(super) fn branch_sites(source: &str) -> Vec<BranchSite> {
    let chars: Vec<char> = source.chars().collect();
    let mut sites = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        skip_noise(&chars, &mut index);
        if index >= chars.len() {
            break;
        }
        let keyword = if word_at(&chars, index, "if") {
            Some("if")
        } else if word_at(&chars, index, "while") {
            Some("while")
        } else {
            None
        };
        if let Some(keyword) = keyword {
            if let Some(site) = condition_after_keyword(&chars, index + keyword.len(), sites.len())
            {
                let end_char = site.end_char;
                sites.push(site);
                index = end_char;
                continue;
            }
        }
        index += 1;
    }
    sites
}

/// Parses the condition following a branch keyword.
fn condition_after_keyword(chars: &[char], mut index: usize, id: usize) -> Option<BranchSite> {
    skip_noise(chars, &mut index);
    if index >= chars.len() {
        return None;
    }
    let parenthesised = chars[index] == '(';
    let start = if parenthesised { index + 1 } else { index };
    let end = if parenthesised {
        matching_paren(chars, index)?
    } else {
        unparenthesised_condition(chars, start)?
    };
    if start >= end {
        return None;
    }
    Some(BranchSite {
        id,
        line: chars[..start].iter().filter(|ch| **ch == '\n').count() + 1,
        col: chars[..start]
            .iter()
            .rev()
            .take_while(|ch| **ch != '\n')
            .count()
            + 1,
        start_char: start,
        end_char: end,
        parenthesised,
    })
}

/// Finds the matching close paren, skipping noise and nested brackets.
fn matching_paren(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = open;
    while index < chars.len() {
        if index < chars.len() {
            let mut probe = index;
            skip_noise(chars, &mut probe);
            if probe != index {
                index = probe;
                if index >= chars.len() {
                    break;
                }
            }
        }
        match chars[index] {
            '(' => depth += 1,
            '[' | '{' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

/// Finds the `{` that ends an unparenthesised condition.
fn unparenthesised_condition(chars: &[char], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = start;
    while index < chars.len() {
        let mut probe = index;
        skip_noise(chars, &mut probe);
        if probe != index {
            index = probe;
            if index >= chars.len() {
                break;
            }
        }
        match chars[index] {
            '(' | '[' => depth += 1,
            '{' => {
                if depth == 0 {
                    return Some(index);
                }
                depth += 1;
            }
            ')' | ']' => {
                depth -= 1;
                if depth == usize::MAX {
                    return None;
                }
            }
            '}' => {
                depth -= 1;
                if depth == usize::MAX {
                    return None;
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

/// Whitespace or comments occupy no token position.
pub(super) fn skip_noise(chars: &[char], index: &mut usize) {
    loop {
        if skip_whitespace(chars, index)
            || skip_line_comment(chars, index)
            || skip_block_comment(chars, index)
            || skip_quoted(chars, index)
        {
            continue;
        }
        break;
    }
}

/// Consumes leading whitespace; reports whether it did.
fn skip_whitespace(chars: &[char], index: &mut usize) -> bool {
    let start = *index;
    while *index < chars.len() && chars[*index].is_whitespace() {
        *index += 1;
    }
    *index > start
}

/// Consumes a `//` comment; reports whether it did.
fn skip_line_comment(chars: &[char], index: &mut usize) -> bool {
    if *index + 1 >= chars.len() || chars[*index] != '/' || chars[*index + 1] != '/' {
        return false;
    }
    while *index < chars.len() && chars[*index] != '\n' {
        *index += 1;
    }
    true
}

/// Consumes a `/* … */` comment; reports whether it did.
fn skip_block_comment(chars: &[char], index: &mut usize) -> bool {
    if *index + 1 >= chars.len() || chars[*index] != '/' || chars[*index + 1] != '*' {
        return false;
    }
    *index += 2;
    while *index + 1 < chars.len() && !(chars[*index] == '*' && chars[*index + 1] == '/') {
        *index += 1;
    }
    *index = (*index + 2).min(chars.len());
    true
}

/// Consumes a quoted string literal; reports whether it did.
fn skip_quoted(chars: &[char], index: &mut usize) -> bool {
    if *index >= chars.len() || !matches!(chars[*index], '"' | '\'' | '`') {
        return false;
    }
    let quote = chars[*index];
    *index += 1;
    while *index < chars.len() {
        if chars[*index] == '\\' {
            *index += 2;
            continue;
        }
        let done = chars[*index] == quote;
        *index += 1;
        if done {
            break;
        }
    }
    true
}

/// Whether `word` begins at `index` as a standalone token.
pub(super) fn word_at(chars: &[char], index: usize, word: &str) -> bool {
    let letters: Vec<char> = word.chars().collect();
    if index + letters.len() > chars.len()
        || !chars[index..index + letters.len()].starts_with(&letters)
    {
        return false;
    }
    let before_ok = index == 0 || !(chars[index - 1].is_alphanumeric() || chars[index - 1] == '_');
    let after = index + letters.len();
    let after_ok = after >= chars.len() || !(chars[after].is_alphanumeric() || chars[after] == '_');
    before_ok && after_ok
}

/// Rewrites branch conditions into probe calls in memory.
pub(super) fn instrument(source: &str, sites: &[BranchSite]) -> Result<String, String> {
    let char_bytes: Vec<usize> = source
        .char_indices()
        .map(|(byte, _)| byte)
        .chain([source.len()])
        .collect();
    let mut output = String::with_capacity(source.len() + sites.len() * 48);
    let mut previous = 0usize;
    for site in sites {
        let start = char_index_to_byte(&char_bytes, site.start_char)?;
        let end = char_index_to_byte(&char_bytes, site.end_char)?;
        output.push_str(&source[previous..start]);
        output.push_str(&format!("{BRANCH_PROBE}({}, ", site.id));
        if site.parenthesised {
            output.push('(');
        }
        output.push_str(&source[start..end]);
        if site.parenthesised {
            output.push(')');
        }
        output.push(')');
        previous = end;
    }
    output.push_str(&source[previous..]);
    if output.contains(BRANCH_PROBE) && source.contains(BRANCH_PROBE) {
        return Err("script already defines the coverage probe name".to_string());
    }
    Ok(output)
}

/// Converts a char index to a byte offset.
pub(super) fn char_index_to_byte(char_bytes: &[usize], index: usize) -> Result<usize, String> {
    char_bytes.get(index).copied().ok_or_else(|| {
        format!(
            "internal branch range {index} exceeds source length {}",
            char_bytes.len() - 1
        )
    })
}
