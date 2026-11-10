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
  `rust-panic` and `rust-msrv`.
- `blaze` build support through `scripts/blaze.sh`, which works around the
  user-namespace restrictions of sandboxed containers.
- The repository dogfoods the tool: every gate runs against this repository in
  `make gate`.
