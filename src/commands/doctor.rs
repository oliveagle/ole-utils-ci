// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `doctor` command: what does this machine look like to the tool?
//!
//! The report is meant to be pasted into a bug report, so it lists the
//! platform, the toolchain versions and the project layout.

use std::path::Path;
use std::process::Command;

use crate::config::Config;
use crate::output::Report;

/// One probed fact about the environment.
#[derive(Debug, Clone)]
pub struct Probe {
    /// What was probed.
    pub name: &'static str,
    /// The answer, or the reason the probe failed.
    pub value: String,
    /// Whether the probe found what it was looking for.
    pub found: bool,
}

/// Probes the environment and returns the findings.
pub fn probe(root: &Path) -> Vec<Probe> {
    let mut probes = vec![
        Probe {
            name: "platform",
            value: format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
            found: true,
        },
        Probe {
            name: "project-root",
            value: root.display().to_string(),
            found: true,
        },
    ];
    for (name, program, args) in [
        ("rustc", "rustc", &["--version"][..]),
        ("cargo", "cargo", &["--version"][..]),
        ("blaze", "blaze", &["--version"][..]),
        ("git", "git", &["--version"][..]),
        ("go", "go", &["version"][..]),
    ] {
        probes.push(command_probe(name, program, args));
    }
    for (name, file) in [
        ("Cargo.toml", "Cargo.toml"),
        ("go.mod", "go.mod"),
        ("VERSION", "VERSION"),
        ("quality-config", ".devine-utils-ci.jsonc"),
    ] {
        probes.push(Probe {
            name,
            value: if root.join(file).exists() {
                "present".to_string()
            } else {
                "absent".to_string()
            },
            found: root.join(file).exists(),
        });
    }
    probes
}

/// Runs one version probe.
fn command_probe(name: &'static str, program: &str, args: &[&str]) -> Probe {
    match Command::new(program).args(args).output() {
        Ok(out) if out.status.success() => Probe {
            name,
            value: String::from_utf8_lossy(&out.stdout).trim().to_string(),
            found: true,
        },
        _ => Probe {
            name,
            value: "not installed".to_string(),
            found: false,
        },
    }
}

/// The `doctor` command.
pub fn doctor(cfg: &Config) -> Result<i32, String> {
    let probes = probe(cfg.root());
    let mut report = Report::new("doctor");
    report.extra = Some(serde_json::json!({
        "probes": probes
            .iter()
            .map(|p| serde_json::json!({
                "name": p.name,
                "value": p.value,
                "found": p.found,
            }))
            .collect::<Vec<_>>(),
    }));
    for p in &probes {
        println!("  {name:<16} {value}", name = p.name, value = p.value);
    }
    report.summary = "doctor: environment inspected".to_string();
    print!("{}", report.render_text());
    Ok(0)
}
