// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Deciding whether a source file is generated code.
//!
//! Two independent signals — either alone is enough:
//!
//! 1. **The filename.** The basename matches one of the well-known Go
//!    codegen suffixes (`.pb.go`, `_grpc.pb.go`, `.pb.gw.go`,
//!    `.pb.validate.go`, `_gen.go`, `.gen.go`, `_generated.go`).
//! 2. **The header sentinel.** The file's first non-blank line matches the
//!    Go convention line `// Code generated … DO NOT EDIT.`.
//!
//! The two signals are OR-ed: matching either makes the file generated. The
//! exclusion side is deliberately permissive — a hand-written file that
//! disappears from the coverage report is a much louder complaint than a
//! generated file that still counts towards the threshold.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::static_regex;

/// The filename suffixes that unconditionally mark a file as generated.
pub const GENERATED_SUFFIXES: [&str; 7] = [
    "_grpc.pb.go",
    ".pb.validate.go",
    ".pb.gw.go",
    ".pb.go",
    "_gen.go",
    ".gen.go",
    "_generated.go",
];

/// The Go-convention "generated" sentinel.
const GENERATED_HEADER: &str = r"^\s*//\s*Code generated .*\bDO NOT EDIT\.";

/// How many non-blank lines the sentinel may sit below.
const HEADER_SCAN_LINES: usize = 32;

/// Whether the basename of `path` is a well-known codegen suffix.
pub fn is_generated_filename(path: &str) -> bool {
    let base = base_name(path);
    GENERATED_SUFFIXES
        .iter()
        .any(|suffix| base.ends_with(suffix))
}

/// `filepath.Base`: the last element of the path.
fn base_name(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(index) => &trimmed[index + 1..],
        None => trimmed,
    }
}

/// Whether the source carries the Go-convention generated sentinel.
///
/// The sentinel must appear before the `package` clause, so the scan stops
/// there; a file with a long licence header above the sentinel still
/// qualifies, because the scan looks at the first 32 non-blank lines.
pub fn has_generated_header(text: &str) -> bool {
    let pattern = static_regex::compile(GENERATED_HEADER);
    let mut seen = 0usize;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if pattern.is_match(line) {
            return true;
        }
        if trimmed.starts_with("package ") {
            return false;
        }
        seen += 1;
        if seen >= HEADER_SCAN_LINES {
            return false;
        }
    }
    false
}

/// Whether the file at `path` is generated code.
///
/// A file that cannot be opened is not generated: the conservative default
/// keeps hand-written code in the coverage numbers.
pub fn is_generated_file(path: &Path) -> bool {
    if is_generated_filename(&path.to_string_lossy()) {
        return true;
    }
    match std::fs::read_to_string(path) {
        Ok(text) => has_generated_header(&text),
        Err(_) => false,
    }
}

/// A cache of the generated-code verdicts.
///
/// The coverage tooling looks at the same file once per block of the
/// profile, so the verdicts are remembered per path.
pub struct Matcher {
    cache: HashMap<String, bool>,
    filesystem_root: PathBuf,
}

impl Matcher {
    /// A matcher for the module rooted at `filesystem_root`.
    pub fn new(filesystem_root: &Path) -> Self {
        Matcher {
            cache: HashMap::new(),
            filesystem_root: filesystem_root.to_path_buf(),
        }
    }

    /// Whether the file at `path` is generated.
    ///
    /// A relative path is resolved against the module root, because the
    /// coverprofile names its files by their import path.
    pub fn is_generated(&mut self, path: &str) -> bool {
        if let Some(cached) = self.cache.get(path) {
            return *cached;
        }
        let verdict = if is_generated_filename(path) {
            true
        } else {
            self.header_verdict(path)
        };
        self.cache.insert(path.to_string(), verdict);
        verdict
    }

    /// Whether the file at `path` carries the generated sentinel.
    fn header_verdict(&self, path: &str) -> bool {
        let named = Path::new(path);
        let resolved = if named.is_absolute() {
            named.to_path_buf()
        } else {
            self.filesystem_root.join(named)
        };
        match std::fs::read_to_string(&resolved) {
            Ok(text) => has_generated_header(&text),
            Err(_) => false,
        }
    }
}

/// The filtered coverprofile and the number of entries kept and dropped.
///
/// The `mode:` header is passed through unchanged, so `go tool cover` still
/// recognises the format. An entry whose file spec cannot be parsed is kept
/// verbatim: losing data is worse than counting a generated file.
pub fn filter_cover_profile(content: &str, matcher: &mut Matcher) -> (usize, usize, String) {
    let mut kept = 0usize;
    let mut dropped = 0usize;
    let mut out = String::new();
    let mut first = true;
    for line in content.lines() {
        if first {
            first = false;
            if line.trim().starts_with("mode:") {
                out.push_str(line);
                out.push('\n');
                continue;
            }
        }
        if line.is_empty() {
            continue;
        }
        match line.rfind(':') {
            Some(index) if index > 0 => {
                let file = &line[..index];
                if matcher.is_generated(file) {
                    dropped += 1;
                    continue;
                }
                kept += 1;
            }
            _ => {}
        }
        out.push_str(line);
        out.push('\n');
    }
    (kept, dropped, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_filename_signal_spots_the_codegen_suffixes() {
        assert!(is_generated_filename("pkg/api.pb.go"));
        assert!(is_generated_filename("pkg/api_grpc.pb.go"));
        assert!(is_generated_filename("pkg/api.pb.gw.go"));
        assert!(is_generated_filename("pkg/api.pb.validate.go"));
        assert!(is_generated_filename("pkg/thing_gen.go"));
        assert!(is_generated_filename("pkg/thing.gen.go"));
        assert!(is_generated_filename("pkg/thing_generated.go"));
        assert!(!is_generated_filename("pkg/thing.go"));
    }

    #[test]
    fn the_header_signal_reads_the_go_convention_line() {
        assert!(has_generated_header(
            "// Code generated by protoc-gen-go. DO NOT EDIT.\n\npackage api\n"
        ));
        assert!(!has_generated_header("package api\n"));
        assert!(!has_generated_header(
            "// Code generated by hand.\npackage api\n"
        ));
    }

    #[test]
    fn the_sentinel_must_come_before_the_package_clause() {
        let text = "package api\n\n// Code generated by tool. DO NOT EDIT.\n";
        assert!(!has_generated_header(text));
    }

    #[test]
    fn a_coverprofile_loses_its_generated_entries() {
        let dir = tempfile::tempdir().expect("temp dir");
        let content =
            "mode: set\nexample.com/a/api.pb.go:1.1,2.2 1 1\nexample.com/a/thing.go:3.3,4.4 1 0\n";
        let mut matcher = Matcher::new(dir.path());
        let (kept, dropped, filtered) = filter_cover_profile(content, &mut matcher);
        assert_eq!(kept, 1);
        assert_eq!(dropped, 1);
        assert!(filtered.starts_with("mode: set\n"));
        assert!(filtered.contains("thing.go"));
        assert!(!filtered.contains("api.pb.go"));
    }

    #[test]
    fn an_unparsable_entry_is_kept_verbatim() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut matcher = Matcher::new(dir.path());
        let (kept, dropped, filtered) = filter_cover_profile("no-colon-here\n", &mut matcher);
        // An entry the parser cannot read is kept, but not counted.
        assert_eq!(kept, 0);
        assert_eq!(dropped, 0);
        assert_eq!(filtered, "no-colon-here\n");
    }
}
