// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Source scanners for the Rust gates.
//!
//! These are the gates a Go repository never had: rules that only make sense
//! for Rust code, checked by reading the sources rather than by running the
//! toolchain.

use std::collections::BTreeMap;
use std::path::Path;

use crate::fsutil;

/// A rule the Rust scanners enforce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// An `unsafe` block or an unsafe function.
    Unsafe,
    /// A `.unwrap()` call.
    Unwrap,
    /// A `.expect()` call.
    Expect,
    /// A `panic!`, `unreachable!` or `todo!` macro.
    PanicMacro,
    /// A `println!`/`dbg!`-style debug macro left in production code.
    DebugMacro,
    /// A `dbg!` macro.
    DbgMacro,
}

impl Rule {
    /// The stable identifier of the rule.
    pub fn id(self) -> &'static str {
        match self {
            Rule::Unsafe => "rust/unsafe",
            Rule::Unwrap => "rust/unwrap",
            Rule::Expect => "rust/expect",
            Rule::PanicMacro => "rust/panic-macro",
            Rule::DebugMacro => "rust/debug-macro",
            Rule::DbgMacro => "rust/dbg-macro",
        }
    }

    /// A one-line description of what the rule forbids.
    pub fn message(self) -> &'static str {
        match self {
            Rule::Unsafe => "unsafe code is not allowed here",
            Rule::Unwrap => "unwrap() may panic; handle the error instead",
            Rule::Expect => "expect() may panic; handle the error instead",
            Rule::PanicMacro => "panic!/unreachable!/todo! is not allowed here",
            Rule::DebugMacro => "debug print left in production code",
            Rule::DbgMacro => "dbg! left in production code",
        }
    }
}

/// One place where a rule was broken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustFinding {
    /// Path relative to the project root.
    pub path: String,
    /// 1-based line number.
    pub line: usize,
    /// The rule that was broken.
    pub rule: Rule,
    /// The source line that broke it.
    pub snippet: String,
}

/// The rules that are enabled by default.
pub const DEFAULT_RULES: [Rule; 3] = [Rule::Unsafe, Rule::PanicMacro, Rule::DbgMacro];

/// Scans the Rust sources under `root` for the given rules.
///
/// Test files and `#[cfg(test)]` modules are skipped: a test may reasonably
/// unwrap or panic.
pub fn scan(root: &Path, rules: &[Rule]) -> Vec<RustFinding> {
    let mut findings = Vec::new();
    for entry in fsutil::walk_files(root) {
        if entry.extension() != ".rs" {
            continue;
        }
        if is_test_file(&entry.rel) {
            continue;
        }
        let Some(data) = fsutil::read_file(&entry.path) else {
            continue;
        };
        let source = String::from_utf8_lossy(&data);
        findings.extend(scan_source(&entry.rel, &source, rules));
    }
    findings
}

/// Whether the file is a test file.
///
/// A module may keep its tests next to itself in a `tests/` directory
/// (`src/commands/upgrade/tests/release.rs`), so any `tests` path component
/// counts, not only the crate-level one.
pub fn is_test_file(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let in_test_dir = rel.split('/').any(|component| component == "tests");
    let named_like_a_test =
        name == "tests.rs" || name.starts_with("test_") || name.ends_with("_test.rs");
    named_like_a_test || in_test_dir
}

/// Scans one source text for the given rules.
///
/// Comments are prose, so a rule never fires on a line that only mentions
/// what it forbids. A line may also opt out with `//nolint`, which skips
/// every rule, or `//nolint:rust/expect`, which skips one.
pub fn scan_source(path: &str, source: &str, rules: &[Rule]) -> Vec<RustFinding> {
    let mut findings = Vec::new();
    let mut in_test_module = false;
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[cfg(test)]") {
            in_test_module = true;
        }
        if in_test_module || trimmed.starts_with("//") {
            continue;
        }
        for rule in rules {
            if !matches_rule(trimmed, *rule) || has_nolint_skip(trimmed, *rule) {
                continue;
            }
            findings.push(RustFinding {
                path: path.to_string(),
                line: index + 1,
                rule: *rule,
                snippet: trimmed.to_string(),
            });
        }
    }
    findings
}

/// Whether the line carries a `nolint` comment that skips `rule`.
fn has_nolint_skip(line: &str, rule: Rule) -> bool {
    let Some(index) = line.find("//") else {
        return false;
    };
    let comment = line[index + 2..].trim_start();
    let Some(rest) = comment.strip_prefix("nolint") else {
        return false;
    };
    let ids = rest.trim_start_matches(':').trim();
    if ids.is_empty() {
        return true;
    }
    ids.split(',').any(|id| id.trim() == rule.id())
}

/// Whether the line breaks `rule`.
///
/// This table names every pattern the gate forbids, so its own lines opt out
/// with `nolint`: the scanner must be able to describe what it rejects
/// without rejecting itself.
fn matches_rule(line: &str, rule: Rule) -> bool {
    match rule {
        Rule::Unsafe => line.starts_with("unsafe ") || line.contains(" unsafe fn"), //nolint
        Rule::Unwrap => line.contains(".unwrap()"),                                 //nolint
        Rule::Expect => line.contains(".expect("),                                  //nolint
        Rule::PanicMacro => {
            line.contains("panic!(") //nolint:rust/panic-macro
                || line.contains("unreachable!(") //nolint:rust/panic-macro
                || line.contains("todo!(") //nolint:rust/panic-macro
                || line.contains("unimplemented!(") //nolint:rust/panic-macro
        }
        Rule::DebugMacro => {
            line.contains("println!(") //nolint:rust/debug-macro
                || line.contains("eprintln!(") //nolint:rust/debug-macro
                || line.contains("dbg!(") //nolint:rust/dbg-macro
                || line.contains("print!(") //nolint:rust/debug-macro
        }
        Rule::DbgMacro => line.contains("dbg!("), //nolint:rust/dbg-macro
    }
}

/// Groups findings by rule so a report can carry one count per rule.
pub fn count_by_rule(findings: &[RustFinding]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for f in findings {
        *counts.entry(f.rule.id()).or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_unsafe_blocks() {
        let src = "fn f() {\n    unsafe { *ptr = 1; }\n}\n";
        let found = scan_source("a.rs", src, &[Rule::Unsafe]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 2);
    }

    #[test]
    fn skips_test_modules() {
        let src = "#[cfg(test)]\nmod tests {\n    fn t() { panic!(\"x\"); }\n}\n";
        assert!(scan_source("a.rs", src, &[Rule::PanicMacro]).is_empty());
    }

    #[test]
    fn skips_comments() {
        let src = "/// A `.unwrap()` call.\n// panic!(\"later\")\nfn f() {}\n";
        assert!(scan_source("a.rs", src, &[Rule::Unwrap, Rule::PanicMacro]).is_empty());
    }

    #[test]
    fn honours_a_nolint_comment() {
        let blanket = "fn f() { todo!() } //nolint\n";
        assert!(scan_source("a.rs", blanket, &[Rule::PanicMacro]).is_empty());
        let specific = "fn f() { todo!() } //nolint:rust/panic-macro\n";
        assert!(scan_source("a.rs", specific, &[Rule::PanicMacro]).is_empty());
        let other = "fn f() { todo!() } //nolint:rust/unsafe\n";
        assert_eq!(scan_source("a.rs", other, &[Rule::PanicMacro]).len(), 1);
    }

    #[test]
    fn recognises_test_files() {
        assert!(!is_test_file("src/lib.rs"));
        assert!(is_test_file("tests/integration.rs"));
        assert!(is_test_file("src/foo_test.rs"));
        assert!(is_test_file("src/cli/tests.rs"));
        assert!(is_test_file("src/commands/upgrade/tests/release.rs"));
        assert!(!is_test_file("src/main.rs"));
    }
}
