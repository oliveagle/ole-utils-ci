// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The top-level usage text.
//!
//! Go prints one hand-written block for `help`, `-h`, `--help` and an
//! unknown command. This is that block, verbatim, plus a short tail
//! that documents the commands and flags this Rust build adds on top
//! of the Go tool. Keeping Go's text intact matters: it is the
//! documentation people copy out of `--help`.

mod commands;

/// The header: title and the usage line.
const HEADER: &str = r#"devine-utils-ci — Unified CI Quality Gate Tool

Usage:
  devine-utils-ci <command> [flags]

"#;

/// The block a bare invocation prints.
///
/// The full block is several hundred lines, and a caller that only wants to
/// remember the shape of the command line pays for all of it in context. This
/// one names the groups, the commands people actually type, and where the
/// rest lives.
const SUMMARY: &str = r#"devine-utils-ci — Unified CI Quality Gate Tool

Usage:
  devine-utils-ci <group> <command> [flags]

Groups:
  golang   Go toolchain, mutation testing and the Go-specific gates.
  rust     The Rust gates: fmt, clippy, test, build, doc, audit, policies.
  tengo    The Tengo tool gates: unit tests, coverage and mutation testing.
  proto    The protobuf gates: buf lint, breaking changes and code generation.
  web      The web gate: HTML, CSS, JavaScript and TypeScript.
  common   The language-agnostic gates every project runs.

Everyday:
  devine-utils-ci all                    every gate this repository runs
  devine-utils-ci rust all               every Rust gate, in order
  devine-utils-ci hook install           install pre-commit, commit-msg and pre-push
  devine-utils-ci <group> <command> --help   the flags of one command

  Nothing runs until you name a command. Run `devine-utils-ci help` for the
  full command list and the global flags.
"#;

/// Everything after the command list: global flags, output modes,
/// examples and the per-command walkthroughs.
const REST: &str = r#"Global Flags:
  --project-root <path>         Project root directory (default: current dir)
  --coverage-threshold <n>      Coverage threshold percentage (default: 80)
  --coverage-allowlist <pkg,...> Packages allowed below coverage threshold
  --mode <dev|ci>               Environment (default: ci)
                                  dev  changed crates only under cargo, blaze's
                                       own cache under blaze (it reads -p as a
                                       test filter), rustdoc/audit/coverage
                                       skipped
                                  ci   whole workspace, every gate
  --build-tags <tags>            Build tags for go test/vet/build (e.g. test_no_credis)
  --base-branch <name>          Base branch for incremental diff (default: main)
  --max-file-lines <n>          Global cap for type-aware file budgets (default: 500)
  --max-complexity <n>          Cyclomatic-complexity cap (default: 20)
  --complexity-warn <n>         Cyclomatic-complexity budget; over it fails (default: 15)
  --verbose, -v                 Verbose output
  --junit <path>                JUnit XML output path (test-report)
  --coverage-html <path>        Coverage HTML report path (coverage)
  --coverage-json <path>        Coverage JSON report path (coverage)

Output modes (per-subcommand; see AGENTS.md §9):
  --output <fmt>, -o <fmt>      Structured output format for data-emitting
                                subcommands (repo-status, ...). Choices:
                                  toon  DEFAULT. Token-Oriented Object Notation,
                                        LLM-friendly, ~40-60%% smaller than JSON
                                  json  pretty JSON (2-space indent, canonical)
                                  text  legacy single-line JSON
                                Legacy: --pretty is an alias for --output=json.

Examples:
  devine-utils-ci all                                              # Run all checks
  devine-utils-ci lint                                             # Run linters only
  devine-utils-ci all --mode dev --base-branch main                # Fast local loop
  devine-utils-ci all --mode ci                                     # Full run (CI)
  devine-utils-ci all --build-tags test_no_credis                  # With build tags
  devine-utils-ci coverage --coverage-threshold 85                 # Custom threshold
  devine-utils-ci coverage --coverage-allowlist pkg/foo,pkg/bar    # With allowlist
  devine-utils-ci file-length --max-file-lines 300                 # Custom file length
  devine-utils-ci complexity --complexity-warn 12 --max-complexity 18
  devine-utils-ci package-size --output json
  devine-utils-ci artifact-size --output json
  devine-utils-ci buf-lint                                         # Lint proto files
  devine-utils-ci buf-breaking --against origin/main               # Check breaking changes

Release workflow:
  devine-utils-ci release-start patch                             # Simple model: start release
  devine-utils-ci release-start minor --release-model strict      # Strict model: start release
  devine-utils-ci release-finish                                   # Simple model: tag on main
  devine-utils-ci release-finish --release-model strict            # Strict model: tag on release

Pipeline-Watch (TUI dashboard for GitLab pipelines, auto-refresh):
  devine-utils-ci pipeline-watch                                    # TUI dashboard (default config)
  devine-utils-ci pipeline-watch --interval 5 --per-repo 10        # Custom cadence
  devine-utils-ci pipeline-watch --config ~/.devine/utils/ci/config.yaml
                                                                    # Use explicit config file

  The dashboard is the only output mode: per-repo rows with a rolling
  status strip (jobs count per pipeline, color-coded by status), MR count,
  window summary, and "last updated" age. There is no --once / --json /
  --detail mode by design — pipeline-watch is for live monitoring.

Pipeline-Run (trigger a GitLab pipeline via API):
  devine-utils-ci pipeline-run <project-path>                       # Trigger on main (default)
  devine-utils-ci pipeline-run <project-path> --ref dev             # Trigger on dev branch
  devine-utils-ci pipeline-run <project-path> --var FOO=bar         # With pipeline variables
  devine-utils-ci pipeline-run <project-path> --wait                # Wait for completion
  devine-utils-ci pipeline-run <project-path> --wait --wait-timeout 300  # Custom timeout
  devine-utils-ci pipeline-run <project-path> --token glpat-xxx     # Explicit token
  devine-utils-ci pipeline-run <project-path> --output json         # JSON output

Workflow (named workflows; default = devine-golang-mcp-app):
  devine-utils-ci workflow list                                      # every known workflow
  devine-utils-ci workflow <name> apps                               # named workflow's app catalog
  devine-utils-ci workflow <name> release-rc                         # bump rc + push + verify UAT/PROD
  devine-utils-ci workflow <name> test-rc --wait --poll-version      # dev-test app + /version
  devine-utils-ci workflow <name> integration-rc --app devine-app-integration-01-function --poll-version
  devine-utils-ci workflow <name> promote                            # gated: integration-01 must be on rc;
                                                                     # auto-runs integration-02 on success
  devine-utils-ci workflow <name> status                             # one-shot snapshot
  # the built-in mcp-app workflow is immutable; add more via ~/.devine/utils/workflow.yaml

  Token resolution: --token > $GITLAB_TOKEN > $DEVINE_GIT_GROUP_ACCESS_TOKEN
  > dotvault get DEVINE_GIT_GROUP_ACCESS_KEY.

Captain-CI Test job (mirrors docs/pipeline/ci_template_v0.yaml):
  devine-utils-ci captain-ci-test                                   # Default: ./..., vendored
  devine-utils-ci captain-ci-test --project-root svc                # Test a sub-module
  devine-utils-ci captain-ci-test --coverpkg=./pkg/... --output-dir _artifacts
  devine-utils-ci captain-ci-test --no-vendor                       # For -mod=mod projects
  devine-utils-ci captain-ci-test --skip-tool-install               # For offline CI images

  Produces three artifacts (Captain-CI ingests them via GitLab reports:
  cobertura / coverage_html / junit):
    cover.xml   Cobertura XML for line/branch coverage
    cover.zip   Zipped index.html for the coverage_html report
    junit.xml   JUnit test report (go-junit-report format)
  Also leaves cover.profile + index.html behind for local inspection.

Git commit identity policy (git-author):
  devine-utils-ci git-author                                       # show current policy vs. current repo identity (TOON by default)
  devine-utils-ci git-author show --output=json                    # pretty JSON dump of rules + resolved identity
  devine-utils-ci git-author check                                 # exit 1 if current identity violates the matching rule
  devine-utils-ci git-author fix                                   # rewrite LOCAL (repo-scoped) user.email/name to satisfy the rule

  The built-in rule: any remote whose URL contains "ctripcorp.com" must
  commit as <username>@trip.com with name = <username>. Additional or
  overriding rules can be added via ~/.devine/utils/ci/git-author.yaml
  (see the "rules:" schema in cmd/devine-utils-ci/internal/gitauthor.go).
  git-author fix ONLY writes repo-local config; it never touches --global.


Report bundle (aggregated quality-gate report — HTML + CSV):
  devine-utils-ci report-bundle                                     # scan every check, write reports/quality-gate/
  devine-utils-ci report-bundle --out-dir dist/quality              # custom output directory
  devine-utils-ci report-bundle --generate-coverage                 # also run "go test -coverprofile=coverage.out" and generate coverage/index.html
  devine-utils-ci report-bundle --coverage-profile coverage.out      # use an existing coverprofile
  devine-utils-ci report-bundle --coverage-threshold 85 --exit-zero  # still emit report even when the gate would fail
  arch                                         # run architecture rule checks (cycles, layers, naming)
  arch --output=json                           # machine-readable output
  arch ./pkg/...                               # check specific packages


  Categories (buckets in the index): Repository structure, Code hygiene,
  Complexity & size, CI configuration, Release. Each check writes both
  <command>.html (browseable) and <command>.csv (grep/join friendly),
  plus a top-level summary.csv and index.html that links them.

Pipeline-Watch config example (save as ~/.devine/utils/ci/config.yaml):

    base_url: https://git.dev.sh.ctripcorp.com
    refresh:
      interval_seconds: 10
    per_repo: 5
    repos:
      - group: devine
      - project: infra/devine_k8s

Token resolution (3-tier fallback):
    1. config.token  literal string in YAML
    2. $DEVINE_GIT_GROUP_ACCESS_TOKEN  environment variable
    3. dotvault get DEVINE_GIT_GROUP_ACCESS_KEY  recommended; see AGENTS.md

Key bindings (TUI):
    q / Esc   quit
    r         refresh now
    + / -     interval up / down (5..120 seconds)
    j / down  select next
    k / up    select previous
    o         open selected repo's latest pipeline in browser
"#;

/// What this build adds to the Go usage block.
///
/// Appended rather than woven in, so the Go text stays a prefix of
/// what `--help` prints and a diff against the original stays
/// readable.
const ADDED: &str = r#"
Rust gates (added by this build; the Go tool has none of them):
  rust                 Every Rust gate, in order.
  rust-fmt             cargo fmt --check.
  rust-clippy          cargo clippy --all-targets --all-features -- -D warnings.
  rust-test            cargo test --all-targets.
  rust-build           cargo build --all-targets.
  rust-doc             cargo doc --no-deps.
  rust-audit           Dependency advisories and licence audit.
  rust-manifest        Cargo.toml hygiene.
  rust-unsafe          No unsafe code in production code.
  rust-panic           No unwrap / expect / panic! in production code.
  rust-msrv            The declared MSRV must be supported.

Flags the Go usage block leaves out:
  --dry-run, -n        Print what would change without changing anything.
  --ts, --timestamp    Prefix every output line with a timestamp. The flag
                       goes after the command; DEVINE_UTILS_TS=1 does the
                       same thing from the environment.
"#;

/// The language groups this build adds on top of the Go command list.
const LANGUAGE: &str = r#"
Language groups (the same commands, grouped by the language they gate):
  golang   Go toolchain, mutation testing and the Go-specific gates.
  rust     The Rust gates: fmt, clippy, test, build, doc, audit, policies.
  tengo    The Tengo tool gates: unit tests, coverage and mutation testing.
  proto    The protobuf gates: buf lint, breaking changes and code generation.
  web      The web gate: HTML, CSS, JavaScript and TypeScript.
  common   The language-agnostic gates every project runs.

  devine-utils-ci golang test        # same as devine-utils-ci test
  devine-utils-ci rust clippy        # same as devine-utils-ci rust-clippy
  devine-utils-ci tengo test         # same as devine-utils-ci tengo-test
  devine-utils-ci golang all         # every Go gate, in order

  A group runs nothing until you name a command. `all` runs every gate of
  the group, in order; the other subcommands run one gate.

Standalone per-language binaries (the same behaviour, shorter name):
  ole-utils-ci      the whole tool
  ole-utils-golang  the golang group
  ole-utils-rust    the rust group
  ole-utils-tengo   the tengo group
  ole-utils-proto   the proto group
  ole-utils-web     the web group
  ole-utils-common  the common group
"#;

/// Prints the usage block on standard output, the way `fmt.Print` does.
pub fn print() {
    print!("{HEADER}{}{REST}{ADDED}{LANGUAGE}", commands::COMMANDS);
}

/// The short block a bare invocation prints.
///
/// The full block is a few hundred lines, and a caller that only wants to
/// remember the shape of the command line pays for all of it in context. This
/// one names the groups, the four commands people actually type, and where
/// the rest lives.
pub fn print_summary() {
    print!("{SUMMARY}");
}

/// Prints the usage block of one language group.
///
/// The block names the standalone binary and the subcommand form, so the
/// user can move between the two spellings without reading the manual. It is
/// the same trade as [`print_summary`]: the group lists its own commands and
/// leaves the full surface to `help`.
pub fn print_language(lang: &crate::languages::Language) {
    let binary = crate::languages::binary_name(lang);
    println!("{binary} — the {} gates of devine-utils-ci", lang.title);
    println!();
    println!("Usage:");
    println!("  {binary} <command> [flags]");
    println!("  devine-utils-ci {} <command> [flags]", lang.name);
    println!();
    println!("Commands:");
    println!(
        "  {:<18}Every {} gate, in order.",
        crate::languages::ALL,
        lang.title
    );
    for (sub, _command, description) in lang.commands {
        println!("  {sub:<18}{description}");
    }
    println!();
    println!("  Nothing runs until you name a command.");
    println!("  Run `devine-utils-ci help` for the full command list and the global flags.");
}
