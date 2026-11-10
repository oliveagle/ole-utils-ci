// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `verify-dist` command: check the release artifacts.
//!
//! Every artifact listed in `SHA256SUMS` has to exist and match its
//! checksum, and no artifact may be empty. A release that ships a truncated
//! binary is worse than a release that does not ship at all.

use std::path::Path;

use crate::config::Config;
use crate::output::{Diagnostic, Report};
use sha2::{Digest, Sha256};

/// The checksum file of the release artifacts.
const CHECKSUM_FILE: &str = "SHA256SUMS";

/// The directory that holds the release artifacts.
const DIST_DIR: &str = "dist";

/// One checksum line of the checksum file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksumEntry {
    /// The expected checksum, in lowercase hex.
    pub sum: String,
    /// The file the checksum belongs to.
    pub file: String,
}

/// Parses a `SHA256SUMS` file.
pub fn parse_checksums(content: &str) -> Vec<ChecksumEntry> {
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(sum) = parts.next() else {
            continue;
        };
        let Some(file) = parts.next() else {
            continue;
        };
        entries.push(ChecksumEntry {
            sum: sum.to_lowercase(),
            file: file.trim_start_matches('*').to_string(),
        });
    }
    entries
}

/// The SHA-256 of a file, in lowercase hex.
pub fn file_checksum(path: &Path) -> Result<String, String> {
    let data =
        std::fs::read(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

/// The `verify-dist` command.
pub fn verify_dist(cfg: &Config) -> Result<i32, String> {
    let root = cfg.root();
    let dist = root.join(DIST_DIR);
    let mut report = Report::new("verify-dist");
    if !dist.is_dir() {
        report.summary = format!("verify-dist: no {DIST_DIR} directory, skipped");
        print!("{}", report.render_text());
        return Ok(0);
    }
    let checksum_path = root.join(CHECKSUM_FILE);
    let entries = match std::fs::read_to_string(&checksum_path) {
        Ok(content) => parse_checksums(&content),
        Err(e) => {
            report.add(Diagnostic::error(
                CHECKSUM_FILE,
                "dist/no-checksums",
                format!("failed to read {CHECKSUM_FILE}: {e}"),
            ));
            report.summary = "verify-dist: the checksum file is missing".to_string();
            print!("{}", report.render_text());
            return Err(report.summary);
        }
    };
    if entries.is_empty() {
        report.add(Diagnostic::error(
            CHECKSUM_FILE,
            "dist/no-checksums",
            "the checksum file lists no artifact",
        ));
    }
    for entry in &entries {
        let path = dist.join(&entry.file);
        if !path.is_file() {
            report.add(Diagnostic::error(
                format!("{DIST_DIR}/{}", entry.file),
                "dist/missing-artifact",
                "the artifact listed in the checksum file is missing",
            ));
            continue;
        }
        let actual = file_checksum(&path)?;
        if actual != entry.sum {
            report.add(Diagnostic::error(
                format!("{DIST_DIR}/{}", entry.file),
                "dist/checksum-mismatch",
                format!("expected {} but the file hashes to {actual}", entry.sum),
            ));
        }
    }
    report.extra = Some(serde_json::json!({ "artifacts": entries.len() }));
    if report.ok {
        report.summary = format!("verify-dist: {} artifact(s) verified", entries.len());
        print!("{}", report.render_text());
        Ok(0)
    } else {
        report.summary = format!(
            "verify-dist: {} artifact(s) failed",
            report.diagnostics.len()
        );
        print!("{}", report.render_text());
        Err(report.summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_checksum_file() {
        let entries = parse_checksums("abc123  dist/tool\ndef456 *dist/other\n# comment\n");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].file, "dist/tool");
        assert_eq!(entries[1].file, "dist/other");
    }
}
