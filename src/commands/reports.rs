// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `report-bundle` command: collect the reports of a CI run.
//!
//! A CI job leaves its reports scattered over the workspace. The bundle
//! command copies them into one directory and writes a manifest, so the
//! artifacts can be uploaded as a single archive.
//!
//! Go builds a private `flag.FlagSet` for this command, so its flags never
//! reach the shared parser and its usage block is its own. The flag set and
//! its exit codes are reproduced here; the report tree the command writes is
//! still the manifest bundle rather than Go's HTML dashboard.

use std::path::{Path, PathBuf};

use crate::cli::{report_parse_error, FlagSet, ParseError};
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

/// Where the bundle goes when `--out-dir` names nothing.
const DEFAULT_OUT_DIR: &str = "reports/quality-gate";

/// The options the private `report-bundle` flag set carries.
#[derive(Debug, Default)]
pub struct Options {
    /// Output directory, empty when the flag was not given.
    pub out_dir: String,
    /// Path to an existing coverprofile.
    pub coverage_profile: String,
    /// Run `go test ./... -coverprofile=coverage.out` first.
    pub generate_coverage: bool,
    /// Path to a pre-generated `go tool cover -html` page.
    pub source_html: String,
    /// Coverage threshold percent.
    pub coverage_threshold: f64,
    /// Verbose sub-check output.
    pub verbose: bool,
    /// Skip the checks that need a GitLab token.
    pub skip_remote: bool,
    /// Always exit 0, even when a check failed.
    pub exit_zero: bool,
    /// Include generated code in the coverage aggregation.
    pub include_generated: bool,
}

/// The private flag set, spelled the way Go's `parseReportBundleFlags` does.
pub fn flag_set() -> FlagSet {
    let mut fs = FlagSet::new();
    fs.str_flag("out-dir", "")
        .str_flag("coverage-profile", "")
        .bool_flag("generate-coverage", false)
        .str_flag("coverage-source-html", "")
        .float_flag("coverage-threshold", 80.0)
        .bool_flag("verbose", false)
        .bool_flag("v", false)
        .bool_flag("skip-remote", false)
        .bool_flag("include-generated", false)
        .bool_flag("exit-zero", false)
        .alias("v", "verbose")
        .typ("coverage-source-html", "go tool cover -html")
        .typ("generate-coverage", "go test ./... -coverprofile=coverage.out")
        .describe("out-dir", "output directory (default: reports/quality-gate)")
        .describe("coverage-profile", "path to a Go coverprofile file")
        .describe(
            "generate-coverage",
            "run go test ./... -coverprofile=coverage.out first if no profile supplied",
        )
        .describe(
            "coverage-source-html",
            "path to a pre-generated go tool cover -html output (optional; auto-generated when --generate-coverage is set)",
        )
        .describe(
            "coverage-threshold",
            "coverage threshold percent (used to colour bars and pick the coverage verdict)",
        )
        .describe("verbose", "verbose sub-check output")
        .describe("v", "shorthand for --verbose")
        .describe(
            "skip-remote",
            "skip checks that require network / a GitLab token (glab-ci-lint)",
        )
        .describe(
            "include-generated",
            "include protoc/stringer/mockgen generated code in the coverage aggregation (default: filtered out)",
        )
        .describe(
            "exit-zero",
            "always exit 0 (still writes reports); useful when running as a report-only CI job",
        );
    fs
}

impl Options {
    /// Reads the parsed flags back out of the set.
    pub fn from(fs: &FlagSet) -> Self {
        let flag = |name: &str| {
            fs.get(name)
                .map(|value| value.as_str(""))
                .unwrap_or_default()
        };
        let truth = |name: &str| fs.get(name).map(|v| v.as_bool(false)).unwrap_or(false);
        Self {
            out_dir: flag("out-dir"),
            coverage_profile: flag("coverage-profile"),
            generate_coverage: truth("generate-coverage"),
            source_html: flag("coverage-source-html"),
            coverage_threshold: fs
                .get("coverage-threshold")
                .map(|value| value.as_float(80.0))
                .unwrap_or(80.0),
            verbose: truth("verbose"),
            skip_remote: truth("skip-remote"),
            exit_zero: truth("exit-zero"),
            include_generated: truth("include-generated"),
        }
    }
}

/// Parses the command line and resolves the output directory.
///
/// Returns [`None`] when the parse already answered the command line (help),
/// the way Go's `flag` package does.
pub fn parse(args: &[String]) -> Result<Option<Options>, String> {
    let mut fs = flag_set();
    if let Err(error) = fs.parse(args) {
        report_parse_error(&error, &fs.render_usage("report-bundle"));
        return match error {
            ParseError::Help => Ok(None),
            other => Err(other.to_string()),
        };
    }
    Ok(Some(Options::from(&fs)))
}

/// The directory the bundle is written to.
pub fn bundle_dir(root: &Path, stamp: &str) -> PathBuf {
    root.join("reports").join(format!("bundle-{stamp}"))
}

/// The `report-bundle` command.
pub fn report_bundle(args: &[String]) -> Result<i32, String> {
    let Some(options) = parse(args)? else {
        return Ok(0);
    };
    run(&options)
}

/// Runs the bundle with an already-parsed option set.
pub fn run(options: &Options) -> Result<i32, String> {
    let root = std::env::current_dir()
        .map_err(|e| format!("cannot determine the working directory: {e}"))?;
    let out_dir = if options.out_dir.is_empty() {
        root.join(DEFAULT_OUT_DIR)
    } else {
        let named = Path::new(&options.out_dir);
        if named.is_absolute() {
            named.to_path_buf()
        } else {
            root.join(named)
        }
    };
    let target = out_dir.join("bundle");
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
        "coverage_threshold": options.coverage_threshold,
        "files": collected,
    });
    let manifest_path = target.join("manifest.json");
    let mut manifest_json = serde_json::to_string_pretty(&manifest).unwrap_or_default();
    manifest_json.push('\n');
    std::fs::write(&manifest_path, manifest_json)
        .map_err(|e| format!("failed to write {}: {e}", manifest_path.display()))?;

    if options.verbose {
        println!(
            "report-bundle: threshold {:.0}%",
            options.coverage_threshold
        );
        if options.skip_remote {
            println!("report-bundle: remote checks skipped");
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn the_usage_block_matches_go() {
        let usage = flag_set().render_usage("report-bundle");
        let expected = [
            "Usage of report-bundle:",
            "  -coverage-profile string",
            "    \tpath to a Go coverprofile file",
            "  -coverage-source-html go tool cover -html",
            "    \tpath to a pre-generated go tool cover -html output (optional; auto-generated when --generate-coverage is set)",
            "  -coverage-threshold float",
            "    \tcoverage threshold percent (used to colour bars and pick the coverage verdict) (default 80)",
            "  -exit-zero",
            "    \talways exit 0 (still writes reports); useful when running as a report-only CI job",
            "  -generate-coverage go test ./... -coverprofile=coverage.out",
            "    \trun go test ./... -coverprofile=coverage.out first if no profile supplied",
            "  -include-generated",
            "    \tinclude protoc/stringer/mockgen generated code in the coverage aggregation (default: filtered out)",
            "  -out-dir string",
            "    \toutput directory (default: reports/quality-gate)",
            "  -skip-remote",
            "    \tskip checks that require network / a GitLab token (glab-ci-lint)",
            "  -v\tshorthand for --verbose",
            "  -verbose",
            "    \tverbose sub-check output",
            "",
        ];
        assert_eq!(usage, expected.join("\n"));
    }

    #[test]
    fn a_bad_flag_is_reported_and_refused() {
        let error = parse(&args(&["--nope"])).expect_err("refused");
        assert_eq!(error, "flag provided but not defined: -nope");
    }

    #[test]
    fn help_answers_the_command_line() {
        assert!(parse(&args(&["-h"])).expect("help").is_none());
    }

    #[test]
    fn the_flags_fill_the_option_set() {
        let options = parse(&args(&[
            "--out-dir",
            "reports/x",
            "--coverage-threshold",
            "85.5",
            "-v",
            "--skip-remote",
            "--exit-zero",
            "--include-generated",
        ]))
        .expect("parse")
        .expect("options");
        assert_eq!(options.out_dir, "reports/x");
        assert_eq!(options.coverage_threshold, 85.5);
        assert!(options.verbose && options.skip_remote && options.exit_zero);
        assert!(options.include_generated);
        assert!(!options.generate_coverage);
    }
}
