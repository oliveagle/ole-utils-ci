// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `devine-utils-ci` — the unified CI quality-gate tool.

use std::io::Write;
use std::process::ExitCode;

mod dispatch;

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
    match dispatch::dispatch(&command, &args) {
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

/// Runs a command, prefixing every line of its standard output with a
/// timestamp — the format Go's `tsWriter` produces: `2006-01-02 15:04:05 `
/// in front of each line, including a final line that has no newline.
///
/// The child must not see `DEVINE_UTILS_TS`: this process is already the
/// timestamping wrapper, so letting the variable through would make every
/// child re-exec itself forever. Go only wraps `os.Stdout`, so the child's
/// standard error is passed through untouched.
fn run_timestamped(command: &str, args: &[String]) -> ExitCode {
    let exe =
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("devine-utils-ci"));
    let mut child = std::process::Command::new(exe);
    child.arg(command).args(args);
    child.env_remove("DEVINE_UTILS_TS");
    let Ok(output) = child.output() else {
        eprintln!("Error: failed to run the command");
        return ExitCode::from(1);
    };
    let mut out = std::io::stdout().lock();
    for line in output.stdout.split_inclusive(|&byte| byte == b'\n') {
        let _ = write!(out, "{} ", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
        let _ = out.write_all(line);
    }
    let _ = out.flush();
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(&output.stderr);
    ExitCode::from(output.status.code().unwrap_or(1) as u8)
}

pub(crate) fn print_usage() {
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
  branch-coverage      Branch coverage check (gobco).
  gomut                Go mutation testing (gomut).
  tengo-test           Tengo tool unit tests with fake call_tool mocks.
  tengo-coverage       Tengo line + branch coverage gates.
  tengo-mutate         Tengo source mutation testing.
  all                  The whole quality gate.

Version and release:
  version-check        VERSION, Cargo.toml and the git tag must agree.
  bump-version         Bump the version: patch|minor|major|rc|promote.
  release-check        The gate that runs before a release.
  release              Create the GitLab release for the current tag.
  release-start        Create the release branch and the bump commit.
  release-finish       Tag the merged release and push the tag.
  release-model        Explain the simple and strict release models.
  protect-set          Set branch and tag protections for a project.

Repository:
  doctor               Report the toolchain and the project layout.
  git-author           Enforce the per-repo commit identity policy.
  cleanup              Stop the background helpers and release their ports.
  report-bundle        Collect the reports of a CI run into one directory.
  verify-dist          Check the release artifacts against SHA256SUMS.

More checks:
  ci-lint              Validate the GitLab CI pipeline file.
  glab-ci-lint         Alias of ci-lint.
  ci-go-env            Check Go env and proxy config in the build scripts.
  arch                 Dependency cycles and layer direction.
  buf-lint             The proto naming conventions.
  buf-breaking         Check the protos for breaking changes.
  buf-generate         Generate code from the proto files.
  web-lint             Syntax sanity for the front-end sources.
  fake-metadata-server Stub the Ctrip metadata endpoint for tests.
  kms-stub             Serve a DEV ONLY local Ctrip KMS stand-in.

GitLab:
  pipeline-run         Trigger a GitLab pipeline for a project (needs GITLAB_TOKEN).
  pipeline-watch       TUI dashboard of pipelines, MRs and commits.
  repo-status          Live pipeline status of one project (no cache; default TOON).
  workflow             Codify the release and test flow for a named workflow.
  logs                 Query or tail Clog logs (needs OSG_CLOG_TOKEN).
  captain-ci-test      Reproduce the Captain-CI Test job artifacts.

Downstream tooling:
  ci-recipe            Render or download the downstream CI artifact recipe.
  upgrade              Download and install a specific version from GitLab releases.

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
