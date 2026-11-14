// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Global configuration shared by every subcommand.
//!
//! The Go original keeps this in `internal.Config` and parses it with Go's
//! `flag` package. This module reproduces the same fields, the same defaults
//! and — importantly — the same validation rules: a project may only *tighten*
//! the built-in budgets, never raise them.

use std::path::{Path, PathBuf};

use crate::cli::{FlagSet, ParseError, GLOBAL_USAGE};
use crate::output;

/// Hard quality budgets owned by the tool, not by project configuration.
///
/// CLI flags and `.devine-utils-ci.jsonc` may only select smaller values.
pub mod limits;

/// The "thresholds may only be tightened" validation.
pub mod validate;

/// Quality-gate environment.
///
/// The two environments answer the two questions a repository asks of a gate:
/// `dev` is the loop a developer runs after every save and must be quick, so
/// it scopes the toolchain gates to the crates the branch touched (under
/// cargo; blaze narrows with its own cache) and lets the fast build driver
/// answer; `ci` is the judge and must see everything, so it
/// runs the whole workspace through plain `cargo`. A `dev` run never replaces
/// a `ci` run: it prints the gates it skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Fast and incremental: changed crates only, fast driver, slow gates off.
    Dev,
    /// Full and authoritative: the whole workspace, every gate.
    Ci,
}

impl Mode {
    /// Canonical CLI spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Dev => "dev",
            Mode::Ci => "ci",
        }
    }

    /// Reads a `--mode` value.
    ///
    /// `release` is the name this flag carried before the environment split
    /// and still selects the full run; anything unrecognised does the same,
    /// because a typo must not silently buy a weaker gate.
    pub fn parse(value: &str) -> Mode {
        match value.trim().to_ascii_lowercase().as_str() {
            "dev" => Mode::Dev,
            _ => Mode::Ci,
        }
    }

    /// Whether this is the fast, incremental environment.
    pub fn is_dev(self) -> bool {
        self == Mode::Dev
    }
}

/// The shared configuration for one invocation.
#[derive(Debug, Clone)]
pub struct Config {
    /// Absolute path of the repository being checked.
    pub project_root: PathBuf,
    /// Line-coverage threshold, in percent.
    pub coverage_threshold: i64,
    /// Branch-coverage threshold, in percent.
    pub branch_coverage_threshold: i64,
    /// Print the underlying tool output.
    pub verbose: bool,
    /// Do not change anything; only show what would happen.
    pub dry_run: bool,
    /// Where the JUnit XML report is written.
    pub junit_path: String,
    /// Where the coverage HTML report is written.
    pub coverage_html_path: String,
    /// Where the coverage JSON report is written.
    pub coverage_json_path: String,
    /// Path of an existing coverprofile to reuse instead of running the tests.
    pub cover_profile: String,
    /// The environment this run belongs to
    pub mode: Mode,
    /// Whether `--mode` was given on the command line.
    pub mode_set: bool,
    /// Build tags passed through to the Go toolchain.
    pub build_tags: String,
    /// Packages allowed to sit below the coverage threshold.
    pub coverage_allowlist: Vec<String>,
    /// The `-covermode` value handed to the Go toolchain.
    pub coverage_mode: String,
    /// The diff base used by the incremental mode.
    pub base_branch: String,
    /// Hard line limit for source files.
    pub max_file_lines: usize,
    /// Whether `--max-file-lines` was given explicitly.
    pub max_file_lines_set: bool,
    /// Hard cyclomatic-complexity limit.
    pub max_complexity: usize,
    /// Cyclomatic-complexity warning threshold.
    pub complexity_warn: usize,
    /// `go test -parallel` value; `0` means "use GOMAXPROCS".
    pub test_parallel: usize,
    /// Count generated code in the coverage numbers.
    pub include_generated: bool,
    /// Release flow model: `simple` or `strict`.
    pub release_model: String,
    /// The machine-readable output format requested by the caller.
    pub output: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            project_root: PathBuf::from("."),
            coverage_threshold: 80,
            branch_coverage_threshold: 80,
            verbose: false,
            dry_run: false,
            junit_path: "junit.xml".to_string(),
            coverage_html_path: "coverage.html".to_string(),
            coverage_json_path: "coverage.json".to_string(),
            cover_profile: String::new(),
            mode: Mode::Ci,
            mode_set: false,
            build_tags: String::new(),
            coverage_allowlist: Vec::new(),
            coverage_mode: "set".to_string(),
            base_branch: "main".to_string(),
            max_file_lines: limits::HARD_SOURCE_MAX_LINES,
            max_file_lines_set: false,
            max_complexity: limits::HARD_COMPLEXITY_MAX,
            complexity_warn: limits::HARD_COMPLEXITY_WARN,
            test_parallel: 0,
            include_generated: false,
            release_model: "simple".to_string(),
            output: String::new(),
        }
    }
}

impl Config {
    /// Builds the default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers every global flag on `fs` with its default value.
    pub fn register(&self, fs: &mut FlagSet) {
        fs.str_flag("project-root", self.project_root.to_string_lossy().as_ref())
            .int_flag("coverage-threshold", self.coverage_threshold)
            .bool_flag("verbose", self.verbose)
            .bool_flag("dry-run", self.dry_run)
            .alias("v", "verbose")
            .alias("n", "dry-run")
            .str_flag("junit", &self.junit_path)
            .str_flag("coverage-html", &self.coverage_html_path)
            .str_flag("coverage-json", &self.coverage_json_path)
            .str_flag("coverage-profile", &self.cover_profile)
            .str_flag("mode", self.mode.as_str())
            .str_flag("build-tags", &self.build_tags)
            .str_flag("base-branch", &self.base_branch)
            .int_flag("max-file-lines", self.max_file_lines as i64)
            .int_flag("max-complexity", self.max_complexity as i64)
            .int_flag("complexity-warn", self.complexity_warn as i64)
            .int_flag("branch-coverage-threshold", self.branch_coverage_threshold)
            .int_flag("parallel", self.test_parallel as i64)
            .list_flag("coverage-allowlist", &self.coverage_allowlist)
            .bool_flag("include-generated", self.include_generated)
            .str_flag("covermode", &self.coverage_mode)
            .str_flag("release-model", &self.release_model)
            .str_flag("output", &self.output)
            .alias("o", "output");
    }

    /// Parses `argv` into this configuration.
    ///
    /// Returns the positional arguments that were left over. A refused
    /// command line reproduces Go's stderr: the error line, then the usage
    /// block, before the caller adds its own `Error: ...` line.
    pub fn parse(&mut self, fs: &mut FlagSet, argv: &[String]) -> Result<Vec<String>, String> {
        // Go's flagWasProvided scans the raw argv for the long spelling only.
        self.max_file_lines_set = argv
            .iter()
            .any(|arg| arg == "--max-file-lines" || arg.starts_with("--max-file-lines="));
        self.mode_set = argv.iter().any(|arg| {
            arg == "--mode"
                || arg == "-mode"
                || arg.starts_with("--mode=")
                || arg.starts_with("-mode=")
        });
        if let Err(error) = fs.parse(argv) {
            match &error {
                // `-h` prints the usage block without a leading error line.
                ParseError::Help => eprint!("{GLOBAL_USAGE}"),
                other => {
                    eprintln!("{other}");
                    eprint!("{GLOBAL_USAGE}");
                }
            }
            return Err(error.to_string());
        }
        self.apply(fs);
        self.validate()?;
        let root = self
            .project_root
            .canonicalize()
            .unwrap_or_else(|_| self.project_root.clone());
        if !root.exists() {
            return Err(format!("project root does not exist: {}", root.display()));
        }
        self.project_root = root;
        Ok(fs.args.clone())
    }

    /// Copies the parsed flag values into this configuration.
    fn apply(&mut self, fs: &FlagSet) {
        let get_str = |name: &str, default: &str| {
            fs.get(name)
                .map(|v| v.as_str(default))
                .unwrap_or_else(|| default.to_string())
        };
        let get_int =
            |name: &str, default: i64| fs.get(name).map(|v| v.as_int(default)).unwrap_or(default);
        let get_bool =
            |name: &str, default: bool| fs.get(name).map(|v| v.as_bool(default)).unwrap_or(default);

        self.project_root = PathBuf::from(get_str("project-root", "."));
        self.coverage_threshold = get_int("coverage-threshold", 80);
        self.branch_coverage_threshold = get_int("branch-coverage-threshold", 80);
        self.verbose = get_bool("verbose", false) || get_bool("v", false);
        self.dry_run = get_bool("dry-run", false) || get_bool("n", false);
        self.junit_path = get_str("junit", "junit.xml");
        self.coverage_html_path = get_str("coverage-html", "coverage.html");
        self.coverage_json_path = get_str("coverage-json", "coverage.json");
        self.cover_profile = get_str("coverage-profile", "");
        self.mode = Mode::parse(&get_str("mode", "ci"));
        self.build_tags = get_str("build-tags", "");
        self.coverage_allowlist = fs
            .get("coverage-allowlist")
            .map(|v| v.as_list().to_vec())
            .unwrap_or_default();
        self.coverage_mode = get_str("covermode", "set");
        self.base_branch = get_str("base-branch", "main");
        self.max_file_lines =
            get_int("max-file-lines", limits::HARD_SOURCE_MAX_LINES as i64).max(0) as usize;
        self.max_complexity =
            get_int("max-complexity", limits::HARD_COMPLEXITY_MAX as i64).max(0) as usize;
        self.complexity_warn =
            get_int("complexity-warn", limits::HARD_COMPLEXITY_WARN as i64).max(0) as usize;
        self.test_parallel = get_int("parallel", 0).max(0) as usize;
        self.include_generated = get_bool("include-generated", false);
        self.release_model = get_str("release-model", "simple");
        self.output = get_str("output", "");
    }

    /// Whether this run is the fast, incremental environment.
    pub fn is_dev(&self) -> bool {
        self.mode.is_dev()
    }

    /// The output format the caller asked for.
    pub fn output_mode(&self) -> output::Mode {
        output::parse_mode(&self.output)
    }

    /// Whether the caller asked for a structured (non-text) payload.
    pub fn structured(&self) -> bool {
        !self.output.is_empty()
    }

    /// Prints a message when `--verbose` is on.
    pub fn log_verbose(&self, message: &str) {
        if self.verbose {
            println!("[verbose] {message}");
        }
    }

    /// The project root as a path slice.
    pub fn root(&self) -> &Path {
        &self.project_root
    }
}

#[cfg(test)]
mod tests;
