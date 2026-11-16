// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `license` gate: every source file must carry a licence header.
//!
//! The header does not have to be the very first line: a shell script starts
//! with a shebang, so the gate looks at the first few lines of the file.

use crate::config::Config;
use crate::fsutil;
use crate::gates::GateResult;
use crate::output::{Diagnostic, Report};

/// Extensions that must carry a licence header.
const SCANNED_EXTENSIONS: [&str; 13] = [
    ".go", ".proto", ".ts", ".tsx", ".rs", ".py", ".pyi", ".mojo", ".🔥", ".java", ".kt", ".swift",
    ".sh",
];

/// Directories that are never inspected.
const SKIP_DIRS: [&str; 7] = [
    "vendor",
    "node_modules",
    "reports",
    ".git",
    ".codegraph",
    ".omo",
    "target",
];

/// Well-known licence header openings.
const COMMON_LICENSE_PREFIXES: [&str; 7] = [
    "// Copyright",
    "// Licensed under",
    "// SPDX",
    "# Copyright",
    "# Licensed under",
    "/*\n * Copyright",
    "/*\n * Licensed under",
];

/// How many lines at the top of a file are inspected for the header.
const HEADER_LOOKAHEAD_LINES: usize = 10;

/// Whether the content carries a licence header.
pub fn has_license_header(content: &str) -> bool {
    if COMMON_LICENSE_PREFIXES
        .iter()
        .any(|prefix| content.starts_with(prefix))
    {
        return true;
    }
    let head: String = content
        .lines()
        .take(HEADER_LOOKAHEAD_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    COMMON_LICENSE_PREFIXES.iter().any(|prefix| {
        if prefix.contains('\n') {
            head.contains(prefix)
        } else {
            head.lines().any(|line| line.starts_with(prefix))
        }
    })
}

/// Scans the project for files without a licence header.
pub fn scan(
    project_root: &std::path::Path,
    qcfg: &crate::quality_config::QualityConfig,
) -> Vec<String> {
    let mut missing = Vec::new();
    for entry in fsutil::walk_files(project_root) {
        if SKIP_DIRS
            .iter()
            .any(|d| entry.rel.split('/').any(|p| p == *d))
        {
            continue;
        }
        if qcfg.should_ignore(&entry.rel, "license") {
            continue;
        }
        if !SCANNED_EXTENSIONS.contains(&entry.extension().as_str()) {
            continue;
        }
        let Some(data) = fsutil::read_file(&entry.path) else {
            continue;
        };
        if data.is_empty() {
            continue;
        }
        let raw = String::from_utf8_lossy(&data);
        let content = fsutil::strip_bom(&raw);
        if has_license_header(content) {
            continue;
        }
        missing.push(entry.rel.clone());
    }
    missing
}

/// Runs the gate and returns its result.
pub fn run(cfg: &Config) -> Result<GateResult, String> {
    let qcfg = crate::quality_config::QualityConfig::load(cfg.root())?.unwrap_or_default();
    let missing = scan(cfg.root(), &qcfg);
    let mut report = Report::new("license");
    for path in &missing {
        report.add(Diagnostic::error(
            path.clone(),
            "license-header",
            "missing license header",
        ));
    }
    if report.ok {
        report.summary = "license: all source files carry a header".to_string();
        Ok(GateResult::finish(report))
    } else {
        report.summary = format!(
            "license: {} file(s) missing a license header",
            missing.len()
        );
        let listed: Vec<String> = missing
            .iter()
            .map(|path| format!("  ✗ {path}: missing license header"))
            .collect();
        let text = format!(
            "license check found {} file(s) without license header:\n{}",
            missing.len(),
            listed.join("\n")
        );
        let structured = format!("{} file(s) missing license header", missing.len());
        Ok(GateResult::failed("license", report.summary.clone(), report).failing(text, structured))
    }
}

/// The human-readable body printed when `--output` is not set.
pub fn text_body(result: &GateResult) -> String {
    let mut body = String::from("📜 Checking license headers...\n");
    if result.passed() {
        body.push_str("✅ License check passed\n");
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_common_licence_headers() {
        assert!(COMMON_LICENSE_PREFIXES
            .iter()
            .any(|p| "// Copyright 2026".starts_with(p)));
        assert!(COMMON_LICENSE_PREFIXES
            .iter()
            .any(|p| "/*\n * Copyright x".starts_with(p)));
        assert!(!COMMON_LICENSE_PREFIXES
            .iter()
            .any(|p| "fn main() {}".starts_with(p)));
    }

    #[test]
    fn accepts_a_header_behind_a_shebang() {
        let script = "#!/usr/bin/env bash\n# Copyright 2026\nset -euo pipefail\n";
        assert!(has_license_header(script));
        assert!(!has_license_header(
            "#!/usr/bin/env bash\nset -euo pipefail\n"
        ));
    }
}
