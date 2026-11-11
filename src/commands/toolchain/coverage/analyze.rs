// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The per-package coverage numbers and the remediation text.
//!
//! `go tool cover -func` reports one line per function. The Go original
//! averages the function percentages of a package, compares that average
//! with the threshold and — when a package falls short — names the packages
//! that have to be covered before the gate may pass.

use std::collections::BTreeMap;

/// The coverage of one package.
#[derive(Debug, Default, Clone, Copy)]
pub struct PackageCoverage {
    /// The number of functions the package has.
    pub functions: usize,
    /// The sum of the function percentages.
    pub covered: f64,
}

impl PackageCoverage {
    /// The average coverage of the package, in percent.
    pub fn average(&self) -> f64 {
        if self.functions == 0 {
            0.0
        } else {
            self.covered / self.functions as f64
        }
    }
}

/// The coverage of the whole module.
#[derive(Debug, Default)]
pub struct Coverage {
    /// The coverage of every package, keyed by its import path.
    pub packages: BTreeMap<String, PackageCoverage>,
    /// The overall coverage the tool reported.
    pub total: f64,
}

/// Reads the `go tool cover -func` output.
///
/// The packages are keyed in sorted order, so two runs over the same module
/// print the same list.
pub fn parse(output: &str) -> Coverage {
    let mut coverage = Coverage::default();
    for line in output.split('\n') {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        if line.contains("total:") {
            coverage.total = parse_percent(parts[parts.len() - 1]).unwrap_or(0.0);
            continue;
        }
        let Some(percent) = parse_percent(parts[parts.len() - 1]) else {
            continue;
        };
        let Some(package) = package_of(parts[0]) else {
            continue;
        };
        let entry = coverage.packages.entry(package).or_default();
        entry.functions += 1;
        entry.covered += percent;
    }
    coverage
}

/// The percentage at the end of a line, without its `%` sign.
fn parse_percent(field: &str) -> Option<f64> {
    let text = field.strip_suffix('%').unwrap_or(field);
    text.parse::<f64>().ok()
}

/// The package a file spec belongs to.
///
/// The spec looks like `import/path/file.go:12.34,15.6`; the package is
/// everything before the last `.go:` and the last `/`.
fn package_of(file: &str) -> Option<String> {
    let mut path = file.to_string();
    if let Some(index) = path.rfind(".go:") {
        path.truncate(index);
    }
    if let Some(index) = path.rfind('/') {
        path.truncate(index);
    }
    if path.is_empty() {
        return None;
    }
    Some(path)
}

/// The packages that fall short of the threshold.
///
/// A package on the allowlist is left alone even when it is short.
pub fn failures(coverage: &Coverage, threshold: i64, allowlist: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (package, stat) in &coverage.packages {
        let average = stat.average();
        if average < threshold as f64 && !allowlist.iter().any(|allowed| allowed == package) {
            out.push(format!(
                "  {package}: {average:.1}% < threshold {threshold}%"
            ));
        }
    }
    out
}

/// The complaint the gate raises when the coverage falls short.
///
/// The text names the packages that have to be covered and spells out the
/// one action that is not allowed: lowering the threshold.
pub fn remediation(failures: &[String], total: f64, threshold: i64) -> String {
    let mut out = String::new();
    if !failures.is_empty() {
        out.push_str(&format!(
            "coverage threshold not met by {} package(s):\n{}\n",
            failures.len(),
            failures.join("\n")
        ));
    }
    if total < threshold as f64 {
        out.push_str(&format!(
            "overall coverage {total:.1}% < threshold {threshold}%\n"
        ));
    }
    out.push('\n');
    out.push_str("═══ REQUIRED REMEDIATION (MANDATORY - DO NOT BYPASS) ═══\n");
    out.push_str("  Coverage below threshold means untested code paths exist.\n");
    out.push_str("  ⚠ Do NOT lower the threshold or skip this check.\n");
    out.push_str("  ⚠ Do NOT delete failing tests to \"pass\" coverage.\n");
    out.push_str("  ⚠ Do NOT git push with coverage below threshold.\n");
    out.push_str("  Correct action: Add tests for the uncovered packages/functions.\n");
    out.push_str("  Focus on: edge cases, error paths, and conditional branches.\n");
    out.push_str("═══════════════════════════════════════════════════════════════\n");
    out
}

/// The JSON report the gate writes.
///
/// The keys are sorted and the number keeps Go's spelling, so the file
/// matches what the Go original writes byte for byte.
pub fn json_report(total: f64, threshold: i64) -> String {
    let passed = total >= threshold as f64;
    format!(
        "{{\n  \"passed\": {passed},\n  \"threshold\": {threshold},\n  \"total_coverage\": {}\n}}",
        go_float(total)
    )
}

/// Go's `encoding/json` spelling of a float.
///
/// Go writes `80` where Rust writes `80.0`, and switches to the exponent
/// form outside the range the decimal form covers.
pub fn go_float(value: f64) -> String {
    let absolute = value.abs();
    if absolute != 0.0 && (absolute < 1e-6 || absolute >= 1e21) {
        let text = format!("{value:e}");
        return match text.split_once('e') {
            Some((mantissa, exponent)) if !exponent.starts_with('-') => {
                format!("{mantissa}e+{exponent}")
            }
            _ => text,
        };
    }
    format!("{value}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_package_of_a_file_spec_is_its_import_path() {
        assert_eq!(
            package_of("example.com/a/pkg/thing.go:12.34,15.6").as_deref(),
            Some("example.com/a/pkg")
        );
        assert_eq!(package_of("thing.go:1.1,2.2").as_deref(), Some("thing"));
        assert_eq!(package_of("/thing.go:1.1,2.2"), None);
    }

    #[test]
    fn the_total_line_is_the_overall_coverage() {
        let output =
            "example.com/a/pkg/thing.go:12.34,15.6\tThing\t80.0%\ntotal:\t(statements)\t75.5%\n";
        let coverage = parse(output);
        assert!((coverage.total - 75.5).abs() < 1e-9);
        let stat = coverage.packages.get("example.com/a/pkg").expect("package");
        assert_eq!(stat.functions, 1);
    }

    #[test]
    fn a_package_below_the_threshold_is_named() {
        let output = "example.com/a/low/thing.go:1.1,2.2\tThing\t10.0%\nexample.com/a/high/thing.go:1.1,2.2\tThing\t99.0%\n";
        let coverage = parse(output);
        let failures = failures(&coverage, 80, &[]);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("example.com/a/low"));
    }

    #[test]
    fn the_json_report_uses_go_number_spelling() {
        assert_eq!(
            json_report(80.0, 80),
            "{\n  \"passed\": true,\n  \"threshold\": 80,\n  \"total_coverage\": 80\n}"
        );
        assert!(json_report(12.5, 80).contains("\"total_coverage\": 12.5"));
    }
}
