// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `captain-ci-test` command: reproduce the Captain-CI Test job locally.
//!
//! Rust port of the Go original's `internal/captain_ci.go`. The command runs
//! `go test` with the template's exact coverage flags, funnels the result
//! through `go-junit-report`, converts the profile into Cobertura XML and an
//! HTML report, and archives the HTML into `cover.zip`.
//!
//! Tests are allowed to fail: the template runs the job with
//! `allow_failure: true`, so a red build still produces artifacts. Only a
//! *conversion* step failing is an error.

mod command;
mod tools;

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::runner::Runner;
use crate::zip;

use command::{test_args, test_command};
use tools::ensure;

/// The banner the Go original prints before doing anything.
const BANNER: &str = "🚀 Running Captain-CI Test job (mirrors docs/pipeline/ci_template_v0.yaml)";

/// The knobs `runCaptainCITest` lifts out of the argument list before the
/// shared parser sees anything.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// `--coverpkg`: what `-coverpkg` instruments. Empty means `./...`.
    pub cover_pkg: String,
    /// `--test-packages`: the final `go test` path argument.
    pub test_packages: String,
    /// `--output-dir`: where the four artifacts land.
    pub output_dir: String,
    /// `--no-vendor`: pass `-mod=mod` and skip `go mod vendor`.
    pub no_vendor: bool,
    /// `--skip-tool-install`: trust the toolchain already on `PATH`.
    pub skip_tool_install: bool,
}

/// Lifts the command's own flags out of `args`, leaving the rest for the
/// shared global parser. A value flag with no value falls through to the
/// passthrough list, exactly as the Go loop's `i+1 < len(args)` guard does.
pub fn split_args(args: &[String]) -> (Options, Vec<String>) {
    let mut options = Options::default();
    let mut rest = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if let Some(step) = take_option(arg, args.get(index + 1), &mut options) {
            index += step;
            continue;
        }
        rest.push(arg.clone());
        index += 1;
    }
    (options, rest)
}

/// Consumes one recognised flag and returns how many tokens it used, or
/// `None` when the token is not this command's flag.
fn take_option(arg: &str, next: Option<&String>, options: &mut Options) -> Option<usize> {
    for (name, slot) in [
        ("--coverpkg", &mut options.cover_pkg),
        ("--test-packages", &mut options.test_packages),
        ("--output-dir", &mut options.output_dir),
    ] {
        if let Some(value) = arg
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('='))
        {
            *slot = value.to_string();
            return Some(1);
        }
        if arg == name {
            if let Some(value) = next {
                *slot = value.clone();
                return Some(2);
            }
        }
    }
    match arg {
        "--no-vendor" => {
            options.no_vendor = true;
            Some(1)
        }
        "--skip-tool-install" => {
            options.skip_tool_install = true;
            Some(1)
        }
        _ => None,
    }
}

/// The `devine-utils-ci captain-ci-test` entry point.
pub fn captain_ci_test(cfg: &Config, options: Options) -> Result<i32, String> {
    println!("{BANNER}");
    let options = with_defaults(cfg, options);
    let out_dir = output_dir(cfg, &options.output_dir);
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("failed to prepare output dir {}: {e}", out_dir.display()))?;
    let runner = Runner::new(&cfg.project_root, cfg.verbose);

    if !options.skip_tool_install {
        ensure(&runner).map_err(|e| format!("captain-ci-test: tool bootstrap failed: {e}"))?;
    }
    if !options.no_vendor && !cfg.project_root.join("vendor").exists() {
        println!("  ↳ vendor/ missing — running `go mod vendor`");
        runner
            .run_interactive("go", &["mod", "vendor"])
            .map_err(|e| format!("captain-ci-test: go mod vendor failed: {e}"))?;
    }

    let paths = Artifacts::new(&out_dir);
    run_tests(cfg, &options, &runner, &paths)?;
    convert(&runner, &paths)?;
    report(cfg, &paths);
    Ok(0)
}

/// Fills in the defaults the Go entry point applies after parsing.
fn with_defaults(cfg: &Config, mut options: Options) -> Options {
    if options.cover_pkg.is_empty() {
        options.cover_pkg = "./...".to_string();
    }
    if options.test_packages.is_empty() {
        options.test_packages = "./...".to_string();
    }
    if options.output_dir.is_empty() {
        options.output_dir = cfg.project_root.to_string_lossy().to_string();
    }
    options
}

/// `--output-dir` relative to the project root, made absolute so the shell
/// redirection written into the verbose one-liner cannot drift.
fn output_dir(cfg: &Config, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        candidate
    } else {
        cfg.project_root.join(candidate)
    }
}

/// The five files one run touches.
struct Artifacts {
    profile: PathBuf,
    junit: PathBuf,
    cover_xml: PathBuf,
    cover_zip: PathBuf,
    html: PathBuf,
}

impl Artifacts {
    fn new(dir: &Path) -> Self {
        Artifacts {
            profile: dir.join("cover.profile"),
            junit: dir.join("junit.xml"),
            cover_xml: dir.join("cover.xml"),
            cover_zip: dir.join("cover.zip"),
            html: dir.join("index.html"),
        }
    }

    /// The order the Go original lists the artifacts in.
    fn listed(&self) -> [&PathBuf; 5] {
        [
            &self.junit,
            &self.cover_xml,
            &self.cover_zip,
            &self.html,
            &self.profile,
        ]
    }
}

/// Step 2: `go test … | go-junit-report -set-exit-code > junit.xml`.
///
/// A failing `go test` is reported and then ignored — the artifacts of a red
/// build are exactly what the developer came for.
fn run_tests(
    cfg: &Config,
    options: &Options,
    runner: &Runner,
    paths: &Artifacts,
) -> Result<(), String> {
    println!("  ↳ go test → cover.profile + junit.xml");
    if cfg.verbose {
        println!("[shell] {}", test_command(options, paths, &cfg.build_tags));
    }
    let args = test_args(options, paths, &cfg.build_tags);
    let producer: Vec<&str> = args.iter().map(String::as_str).collect();
    if let Err(error) = runner.run_pipe(
        &producer,
        &["go-junit-report", "-set-exit-code"],
        &paths.junit,
    ) {
        println!(
            "  ⚠ go test wrapper exited non-zero: {error} (continuing so we can convert whatever coverage exists)"
        );
    }
    std::fs::metadata(&paths.profile).map_err(|e| {
        format!(
            "captain-ci-test: cover.profile was not produced (go test never ran?): {}",
            path_error("stat", &paths.profile, &e)
        )
    })?;
    Ok(())
}

/// Steps 3 to 5: the two conversions and the archive.
fn convert(runner: &Runner, paths: &Artifacts) -> Result<(), String> {
    let profile = paths.profile.to_string_lossy().to_string();
    println!("  ↳ gocov convert | gocov-xml → cover.xml");
    runner
        .run_pipe(
            &["gocov", "convert", &profile],
            &["gocov-xml"],
            &paths.cover_xml,
        )
        .map_err(|e| format!("captain-ci-test: cover.xml generation failed: {e}"))?;

    println!("  ↳ gocov convert | gocov-html → index.html");
    runner
        .run_pipe(
            &["gocov", "convert", &profile],
            &["gocov-html"],
            &paths.html,
        )
        .map_err(|e| format!("captain-ci-test: index.html generation failed: {e}"))?;

    println!("  ↳ archive/zip → cover.zip");
    zip::write_zip(&paths.cover_zip, &[("index.html", &paths.html)])
        .map_err(|e| format!("captain-ci-test: cover.zip generation failed: {e}"))
}

/// The closing artifact listing, in the Go original's order.
fn report(cfg: &Config, paths: &Artifacts) {
    println!("✅ Captain-CI test artifacts:");
    for path in paths.listed() {
        if let Ok(metadata) = std::fs::metadata(path) {
            println!(
                "    {} ({} bytes)",
                rel_to(&cfg.project_root, path),
                metadata.len()
            );
        }
    }
}

/// A project-relative display path when `path` sits under `root`.
fn rel_to(root: &Path, path: &Path) -> String {
    let root = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
    let target = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    match target.strip_prefix(&root) {
        Ok(rest) if !rest.as_os_str().is_empty() => rest.display().to_string(),
        Ok(_) => ".".to_string(),
        Err(_) => path.display().to_string(),
    }
}

/// Go's `*os.PathError` wording, which is what a missing artifact reports.
fn path_error(op: &str, path: &Path, error: &std::io::Error) -> String {
    let reason = match error.kind() {
        std::io::ErrorKind::NotFound => "no such file or directory".to_string(),
        std::io::ErrorKind::PermissionDenied => "permission denied".to_string(),
        _ => error.to_string(),
    };
    format!("{op} {}: {reason}", path.display())
}

#[cfg(test)]
mod tests;
