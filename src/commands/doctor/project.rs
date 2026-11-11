// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The checks that read the project: `vendor/`, `go mod verify` and
//! `go mod tidy`.
//!
//! The Go original keeps these two probes at the bottom of
//! `internal/doctor.go`. `checkGoModTidy` runs whether or not the vendor
//! checks passed, so the tidy line is appended here rather than inside the
//! vendor walk.

use std::io::ErrorKind;
use std::path::Path;

use super::checks::{combined, go, Check};
use crate::process::exit_status;

/// `checkVendorConsistency` followed by `checkGoModTidy`, in the order the
/// Go original appends them.
pub(super) fn run(root: &Path) -> Vec<Check> {
    let mut results = vendor(root);
    results.push(go_mod_tidy(root));
    results
}

/// The vendor directory, its manifest and `go mod verify`.
fn vendor(root: &Path) -> Vec<Check> {
    let directory = root.join("vendor");
    if missing(&directory) {
        return vec![Check::warn(
            "Vendor directory",
            "No vendor/ directory found",
            "Run 'devine-utils-ci install' or 'go mod vendor' to create it",
        )];
    }
    let mut results = vec![Check::pass("Vendor directory", "vendor/ exists")];
    if missing(&directory.join("modules.txt")) {
        results.push(Check::fail(
            "  modules.txt",
            "vendor/modules.txt not found — vendor is incomplete",
            "Run 'go mod vendor' to regenerate the vendor directory",
        ));
        return results;
    }
    results.push(Check::pass("  modules.txt", "vendor/modules.txt exists"));
    results.push(go_mod_verify(root));
    results
}

/// Whether `path` is absent, which is the only `os.Stat` failure Go's
/// `os.IsNotExist` accepts; every other outcome counts as "there".
fn missing(path: &Path) -> bool {
    matches!(
        std::fs::metadata(path),
        Err(error) if error.kind() == ErrorKind::NotFound
    )
}

/// `go mod verify`: a failure is a failure, and a success is judged by the
/// words the tool printed.
fn go_mod_verify(root: &Path) -> Check {
    let outcome = go(root, &["mod", "verify"]);
    let succeeded = matches!(&outcome, Ok(output) if output.status.success());
    let (reported, text) = match outcome {
        Ok(output) => (exit_status(&output.status), combined(&output)),
        Err(error) => (error, String::new()),
    };
    if !succeeded {
        return Check::fail(
            "  go mod verify",
            format!("Module verification failed: {reported}\n{text}"),
            "Run 'go mod tidy && go mod vendor' to fix module inconsistencies",
        );
    }
    Check {
        name: "  go mod verify".to_string(),
        status: verify_status(&text),
        detail: verify_detail(&text),
        hint: String::new(),
    }
}

/// `go mod tidy -e`, whose success line ignores whatever the tool printed.
fn go_mod_tidy(root: &Path) -> Check {
    let outcome = go(root, &["mod", "tidy", "-e"]);
    let succeeded = matches!(&outcome, Ok(output) if output.status.success());
    let (reported, text) = match outcome {
        Ok(output) => (exit_status(&output.status), combined(&output)),
        Err(error) => (error, String::new()),
    };
    if succeeded {
        return Check::pass("go mod tidy", "go.mod and go.sum are consistent");
    }
    Check::fail(
        "go mod tidy",
        format!("go mod tidy failed: {reported}\n{text}"),
        "Run 'go mod tidy' to fix dependency issues, then 'go mod vendor'",
    )
}

/// One line per output line, `; `-joined, with the empty output Go
/// replaces with `All modules verified`.
pub(super) fn verify_detail(output: &str) -> String {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return "All modules verified".to_string();
    }
    trimmed.split('\n').collect::<Vec<_>>().join("; ")
}

/// A successful `go mod verify` still fails when any line reports an
/// `error` or a `mismatch`.
pub(super) fn verify_status(output: &str) -> &'static str {
    let suspicious = output
        .trim()
        .split('\n')
        .any(|line| line.contains("error") || line.contains("mismatch"));
    if suspicious {
        "fail"
    } else {
        "pass"
    }
}
