// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! JSONC comment stripping and the glob matcher used by the ignore rules.

/// Strips `//` and `/* */` comments from JSONC, respecting string literals.
pub fn strip_jsonc_comments(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '"' {
            i = copy_string_literal(&chars, i, &mut out);
            continue;
        }
        if starts_with(&chars, i, "//") {
            i = skip_line_comment(&chars, i);
            continue;
        }
        if starts_with(&chars, i, "/*") {
            i = skip_block_comment(&chars, i);
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Whether the characters at `index` start with `needle`.
fn starts_with(chars: &[char], index: usize, needle: &str) -> bool {
    let needle: Vec<char> = needle.chars().collect();
    chars[index..].starts_with(&needle[..])
}

/// Copies a quoted string literal, escapes included, into `out`.
fn copy_string_literal(chars: &[char], start: usize, out: &mut String) -> usize {
    let mut i = start;
    out.push('"');
    i += 1;
    while i < chars.len() {
        let ch = chars[i];
        out.push(ch);
        i += 1;
        if ch == '\\' {
            if i < chars.len() {
                out.push(chars[i]);
                i += 1;
            }
        } else if ch == '"' {
            break;
        }
    }
    i
}

/// Skips a `//` comment and returns the index of the newline that ends it.
fn skip_line_comment(chars: &[char], start: usize) -> usize {
    let mut i = start + 2;
    while i < chars.len() && chars[i] != '\n' {
        i += 1;
    }
    i
}

/// Skips a `/* */` comment and returns the index just past its closing brace.
fn skip_block_comment(chars: &[char], start: usize) -> usize {
    let mut i = start + 2;
    while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
        i += 1;
    }
    (i + 2).min(chars.len())
}

/// Matches a path against a glob pattern.
///
/// `*` matches within one path segment, `**` crosses segments and `?` matches
/// a single character.
pub fn match_glob_pattern(path: &str, pattern: &str) -> bool {
    let path = path.replace('\\', "/");
    let pattern = pattern.replace('\\', "/");
    if pattern.contains("**") {
        return match_double_star(&path, &pattern);
    }
    glob_match(&pattern, &path)
}

/// Matches a `**` pattern segment by segment.
///
/// A `**` segment matches zero or more path segments, so `src/**/*.go`
/// matches `src/a.go` as well as `src/deep/nested/a.go`.
fn match_double_star(path: &str, pattern: &str) -> bool {
    let pat_segments: Vec<&str> = pattern.split('/').collect();
    let path_segments: Vec<&str> = path.split('/').collect();
    match_segments(&path_segments, &pat_segments)
}

/// Matches a path segment list against a pattern segment list.
fn match_segments(path: &[&str], pattern: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    if pattern[0] == "**" {
        return (0..=path.len()).any(|i| match_segments(&path[i..], &pattern[1..]));
    }
    if path.is_empty() || !glob_match(pattern[0], path[0]) {
        return false;
    }
    match_segments(&path[1..], &pattern[1..])
}

/// A minimal glob matcher supporting `*` and `?` within one path segment.
///
/// Like Go's `filepath.Match`, a `*` never crosses a path separator.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_match_chars(&p, &t)
}

/// Matches the pattern against the text with the classic backtracking
/// algorithm: a `*` remembers where it started so it can extend on failure.
fn glob_match_chars(p: &[char], t: &[char]) -> bool {
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while ti < t.len() {
        if let Some((next_pi, next_ti)) = match_single(p, t, pi, ti) {
            pi = next_pi;
            ti = next_ti;
        } else if pi < p.len() && p[pi] == '*' {
            star = pi;
            mark = ti;
            pi += 1;
        } else if star != usize::MAX && t[mark] != '/' {
            pi = star + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Matches one `?` or one literal character.
fn match_single(p: &[char], t: &[char], pi: usize, ti: usize) -> Option<(usize, usize)> {
    if pi >= p.len() {
        return None;
    }
    if p[pi] == '?' {
        if t[ti] == '/' {
            return None;
        }
        return Some((pi + 1, ti + 1));
    }
    if p[pi] == t[ti] {
        return Some((pi + 1, ti + 1));
    }
    None
}

/// Matches a package path against a pattern.
///
/// `internal/legacy/...` matches `internal/legacy` and everything under it.
pub fn match_package_pattern(pkg: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/...") {
        return pkg == prefix || pkg.starts_with(&format!("{prefix}/"));
    }
    pkg == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_line_and_block_comments() {
        let input = "{\n  // a comment\n  \"a\": 1 /* inline */, \"b\": \"// not a comment\"\n}";
        let cleaned = strip_jsonc_comments(input);
        assert!(!cleaned.contains("// a comment"));
        assert!(!cleaned.contains("inline"));
        assert!(cleaned.contains("// not a comment"));
    }

    #[test]
    fn matches_globs() {
        assert!(match_glob_pattern("src/a.go", "src/*.go"));
        assert!(!match_glob_pattern("src/deep/a.go", "src/*.go"));
        assert!(match_glob_pattern("src/deep/a.go", "src/**/*.go"));
        assert!(match_glob_pattern("src/deep/a.go", "**/a.go"));
        assert!(match_glob_pattern("anything", "**"));
    }

    #[test]
    fn matches_package_patterns() {
        assert!(match_package_pattern("internal/legacy", "internal/legacy"));
        assert!(match_package_pattern(
            "internal/legacy/foo",
            "internal/legacy/..."
        ));
        assert!(!match_package_pattern(
            "internal/other",
            "internal/legacy/..."
        ));
    }
}
