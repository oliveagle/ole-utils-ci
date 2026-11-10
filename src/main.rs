// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `devine-utils-ci` — the unified CI quality-gate tool.

use std::io::Write;
use std::process::ExitCode;

use devine_utils_ci::commands;
use devine_utils_ci::config::Config;
use devine_utils_ci::version;

/// The commands that need nothing but the global flags.
const GATE_COMMANDS: [&str; 12] = [
    "bugs",
    "sanity",
    "license",
    "structure",
    "file-length",
    "complexity",
    "package-size",
    "artifact-size",
    "go-pin",
    "goversion",
    "no-shell-exec",
    "plugin-freshness",
];

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (command, args, timestamped) = split_args(&argv);
    if command.is_empty() {
        print_usage();
        return ExitCode::from(1);
    }
    if timestamped {
        return run_timestamped(&command, &args);
    }
    match dispatch(&command, &args) {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(1)
        }
    }
}

/// Splits the command line into the subcommand, its arguments and whether
/// timestamped output was requested.
fn split_args(argv: &[String]) -> (String, Vec<String>, bool) {
    let mut timestamped = std::env::var("DEVINE_UTILS_TS").is_ok_and(|v| !v.is_empty());
    let mut command = String::new();
    let mut args = Vec::new();
    for (index, arg) in argv.iter().enumerate() {
        if arg == "--ts" || arg == "--timestamp" {
            timestamped = true;
            continue;
        }
        if index == 0 && !arg.starts_with('-') {
            command = arg.clone();
            continue;
        }
        args.push(arg.clone());
    }
    (command, args, timestamped)
}

/// Runs a command with every output line prefixed by a timestamp.
fn run_timestamped(command: &str, args: &[String]) -> ExitCode {
    let exe =
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("devine-utils-ci"));
    let mut child = std::process::Command::new(exe);
    child.arg(command).args(args);
    let Ok(output) = child.output() else {
        eprintln!("Error: failed to run the command");
        return ExitCode::from(1);
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut out = std::io::stdout().lock();
    for line in stdout.lines() {
        let _ = writeln!(
            out,
            "[{}] {line}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        );
    }
    let _ = out.flush();
    for line in stderr.lines() {
        let _ = writeln!(
            std::io::stderr(),
            "[{}] {line}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        );
    }
    ExitCode::from(output.status.code().unwrap_or(1) as u8)
}

/// Runs the requested subcommand.
fn dispatch(command: &str, args: &[String]) -> Result<i32, String> {
    if matches!(command, "help" | "-h" | "--help") {
        print_usage();
        return Ok(0);
    }
    if matches!(command, "version" | "--version") {
        println!("{}", devine_utils_ci::version_banner());
        return Ok(0);
    }
    let mut cfg = Config::new();
    if GATE_COMMANDS.contains(&command) {
        commands::gates::parse(&mut cfg, args)?;
        return commands::gates::run_gate(command, &cfg);
    }
    if commands::rust::COMMANDS.contains(&command) {
        commands::rust::parse(&mut cfg, args)?;
        return commands::rust::run(command, &cfg);
    }
    match command {
        "install" => with_config(commands::toolchain::install, &mut cfg, args),
        "lint" => with_config(commands::toolchain::lint, &mut cfg, args),
        "test" => with_config(commands::toolchain::test, &mut cfg, args),
        "build" => with_config(commands::toolchain::build, &mut cfg, args),
        "coverage" => with_config(commands::toolchain::coverage, &mut cfg, args),
        "test-report" => with_config(commands::toolchain::test_report, &mut cfg, args),
        "all" => with_config(commands::all::run, &mut cfg, args),
        "version-check" => with_config(commands::version::version_check, &mut cfg, args),
        "release-check" => with_config(commands::version::release_check, &mut cfg, args),
        "release-finish" => with_config(commands::version::release::release_finish, &mut cfg, args),
        "doctor" => with_config(commands::doctor::doctor, &mut cfg, args),
        "repo-status" => with_config(commands::repo::repo_status, &mut cfg, args),
        "cleanup" => with_config(commands::repo::cleanup, &mut cfg, args),
        "ci-lint" | "glab-ci-lint" => with_config(commands::ci::ci_lint, &mut cfg, args),
        "arch" => with_config(commands::arch::arch, &mut cfg, args),
        "buf-lint" => with_config(commands::proto::buf_lint, &mut cfg, args),
        "web-lint" => with_config(commands::web::web_lint, &mut cfg, args),
        "report-bundle" => with_config(commands::reports::report_bundle, &mut cfg, args),
        "verify-dist" => with_config(commands::dist::verify_dist, &mut cfg, args),
        "bump-version" => bump_command(&mut cfg, args, false),
        "release-start" => bump_command(&mut cfg, args, true),
        other => Err(format!("unknown command: {other}")),
    }
}

/// Parses the global flags for a command, then runs it.
fn with_config(
    run: fn(&Config) -> Result<i32, String>,
    cfg: &mut Config,
    args: &[String],
) -> Result<i32, String> {
    commands::gates::parse(cfg, args)?;
    run(cfg)
}

/// Parses the bump type for `bump-version` and `release-start`.
fn bump_command(cfg: &mut Config, args: &[String], release: bool) -> Result<i32, String> {
    let rest = commands::gates::parse(cfg, args)?;
    let raw = rest.first().map(String::as_str).unwrap_or("");
    let bump = version::parse_bump(raw)?;
    if release {
        commands::version::release::release_start(cfg, bump)
    } else {
        commands::version::bump::bump_version(cfg, bump)
    }
}
fn print_usage() {
    let markers = devine_utils_ci::gates::bugs::MARKERS.join(" / ");
    print!(
        "devine-utils-ci — Unified CI Quality Gate Tool

Usage:
  devine-utils-ci <command> [flags]

Quality gates:
  bugs                 Scan for {markers} markers.
  sanity               Trailing whitespace, conflict markers, missing newlines.
  license              Source files must carry a licence header.
  structure            Repository root layout and build-artifact hygiene.
  file-length          Type-aware line budgets (source/script/html/declarative).
  complexity           Cyclomatic complexity per function.
  package-size         Warning-only package growth budgets.
  artifact-size        Release-artifact size budgets.
  go-pin               Go toolchain pin drift.
  goversion            go.mod versus the installed Go toolchain.
  no-shell-exec        Production code must not shell out to bash/sh.
  plugin-freshness     Plugin .so files must be newer than their sources.

Rust gates:
  rust                 Every Rust gate, in order.
  rust-fmt             cargo fmt --check.
  rust-clippy          cargo clippy --all-targets -- -D warnings.
  rust-test            cargo test.
  rust-build           cargo build --all-targets.
  rust-doc             cargo doc --no-deps.
  rust-audit           Dependency advisories and licence audit.
  rust-manifest        Cargo.toml hygiene.
  rust-unsafe          No unsafe code in production code.
  rust-panic           No unwrap/expect/panic! in production code.
  rust-msrv            The declared MSRV must be supported.

Go toolchain:
  install              go mod vendor.
  lint                 gofmt -s, go mod edit -fmt, go vet, golangci-lint.
  test                 go test with the race detector.
  build                go build ./...
  coverage             Line coverage against the threshold.
  test-report          JUnit XML report.
  all                  The whole quality gate.

Version and release:
  version-check        VERSION, Cargo.toml and the git tag must agree.
  bump-version         Bump the version: patch|minor|major|rc|promote.
  release-check        The gate that runs before a release.
  release-start        Create the release branch and the bump commit.
  release-finish       Tag the merged release and push the tag.

Repository:
  doctor               Report the toolchain and the project layout.
  repo-status          Branch, head, dirty paths and recent commits.
  cleanup              Remove the build artifacts.
  report-bundle        Collect the reports of a CI run into one directory.
  verify-dist          Check the release artifacts against SHA256SUMS.

More checks:
  ci-lint              Validate the GitLab CI pipeline file.
  glab-ci-lint         Alias of ci-lint.
  arch                 Dependency cycles and layer direction.
  buf-lint             The proto naming conventions.
  web-lint             Syntax sanity for the front-end sources.

Global flags:
  --project-root <path>       Repository to check (default: .).
  --mode <dev|release>        dev runs only the changed packages.
  --base-branch <name>        Diff base for the incremental mode (default: main).
  --coverage-threshold <n>    Line-coverage floor (default: 80).
  --branch-coverage-threshold <n>
  --coverage-allowlist <pkg,…>
  --covermode <set|atomic|count>
  --max-file-lines <n>        Source file line ceiling (default: 500).
  --complexity-warn <n>       Complexity warning threshold (default: 15).
  --max-complexity <n>        Complexity hard ceiling (default: 20).
  --build-tags <tags>
  --parallel <n>
  --junit <path>              JUnit XML output path (default: junit.xml).
  --coverage-html <path>
  --coverage-json <path>
  --output <text|json|toon>   Structured output format.
  --verbose, -v               Print the underlying tool output.
  --dry-run, -n               Do not change anything.
  --ts, --timestamp            Prefix every output line with a timestamp.
",
        markers = markers
    );
}
