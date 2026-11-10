// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `report-bundle` command: collect the reports of a CI run.
//!
//! A CI job leaves its reports scattered over the workspace. The bundle
//! command copies them into one directory and writes a manifest, so the
//! artifacts can be uploaded as a single archive.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::output::Report;

/// The report files the bundle collects.
const REPORT_FILES: [&str; 8] = [
    "coverage.out",
    "coverage.json",
    "coverage.html",
    "junit.xml",
    "reports/tengo/test.json",
    "reports/tengo/coverage.json",
    "reports/tengo/mutation.json",
    "reports/tengo/junit.xml",
];

/// The directory the bundle is written to.
pub fn bundle_dir(root: &Path, stamp: &str) -> PathBuf {
    root.join("reports").join(format!("bundle-{stamp}"))
}

/// The `report-bundle` command.
pub fn report_bundle(cfg: &Config) -> Result<i32, String> {
    let root = cfg.root();
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let target = bundle_dir(root, &stamp);
    std::fs::create_dir_all(&target)
        .map_err(|e| format!("failed to create {}: {e}", target.display()))?;

    let mut collected = Vec::new();
    for name in REPORT_FILES {
        let source = root.join(name);
        if !source.is_file() {
            continue;
        }
        let flat = name.replace('/', "__");
        let destination = target.join(&flat);
        std::fs::copy(&source, &destination)
            .map_err(|e| format!("failed to copy {}: {e}", source.display()))?;
        collected.push(serde_json::json!({
            "source": name,
            "bundled_as": flat.clone(),
            "bytes": std::fs::metadata(&destination).map(|m| m.len()).unwrap_or(0),
        }));
    }

    let manifest = serde_json::json!({
        "schema_version": 1,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "files": collected,
    });
    let manifest_path = target.join("manifest.json");
    let mut manifest_json = serde_json::to_string_pretty(&manifest).unwrap_or_default();
    manifest_json.push('\n');
    std::fs::write(&manifest_path, manifest_json)
        .map_err(|e| format!("failed to write {}: {e}", manifest_path.display()))?;

    let mut report = Report::new("report-bundle");
    report.extra = Some(serde_json::json!({
        "bundle": target.display().to_string(),
        "files": collected.len(),
    }));
    println!("📦 Report bundle: {}", target.display());
    for entry in &collected {
        println!("  {} ({})", entry["bundled_as"], entry["bytes"]);
    }
    report.summary = format!("report-bundle: {} file(s) bundled", collected.len());
    print!("{}", report.render_text());
    Ok(0)
}
