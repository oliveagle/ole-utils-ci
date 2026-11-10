# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.11.7] — 2026-11-11

### Added

- Rust rewrite of the Go `devine-utils-ci` binary, keeping the command
  surface, the `--output text|json|toon` contract and the exit-code semantics.
- Rust-specific quality gates: `rust`, `rust-fmt`, `rust-clippy`, `rust-test`,
  `rust-build`, `rust-doc`, `rust-audit`, `rust-manifest`, `rust-unsafe`,
  `rust-panic` and `rust-msrv`. The Rust gates are driven by `blaze` when it is
  installed, falling back to `cargo` otherwise.
- Version and release commands: `version-check`, `bump-version`,
  `release-check`, `release-start` and `release-finish`.
- Repository commands: `doctor`, `repo-status`, `cleanup` (with `--dry-run`),
  `report-bundle` and `verify-dist`.
- Extra checks: `ci-lint` / `glab-ci-lint`, `arch`, `buf-lint` and `web-lint`.
- Integration tests in `tests/cli.rs` that drive the compiled binary and pin
  the exit-code, text and JSON contracts.
- `blaze` build support through `scripts/blaze.sh`, which works around the
  user-namespace restrictions of sandboxed containers.
- The repository dogfoods the tool: every gate runs against this repository in
  `make gate`.

### Fixed

- `bump-version --dry-run` and `release-start --dry-run` no longer write
  anything; the dry-run check happens before any filesystem or git change.
- Global flags are now accepted after positional arguments, so
  `bump-version patch --dry-run` no longer ignores the flag.
- `repo-status` reported the wrong removal count in `cleanup`, and `cleanup`
  now honours `--dry-run` in what it prints.
- `ci-lint`, `go-pin`, `goversion`, `buf-lint` and `verify-dist` skip cleanly
  when the input they need is absent instead of failing the run.
- The `report-bundle` manifest ends with a newline, and generated `reports/`
  output is excluded from the source-scanning gates.
