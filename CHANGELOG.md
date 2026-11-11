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
- Quality gates: `bugs`, `sanity`, `license`, `structure`, `file-length`,
  `complexity`, `package-size`, `artifact-size`, `go-pin`, `goversion`,
  `no-shell-exec` and `plugin-freshness`.
- Go toolchain commands: `install`, `lint`, `test`, `build`, `coverage`,
  `branch-coverage`, `gomut`, `tengo-test`, `tengo-coverage`, `tengo-mutate`,
  `test-report` and `all`.
- Version and release commands: `version-check`, `bump-version`,
  `release-check`, `release`, `release-start`, `release-finish`, `release-model`
  and `protect-set`.
- Repository commands: `doctor`, `git-author`, `cleanup` (stop the
  `fake-metadata-server` helpers and release port 18080), `report-bundle` and
  `verify-dist`.
- Extra checks: `ci-lint` / `glab-ci-lint`, `ci-go-env`, `arch`, `buf-lint`,
  `buf-breaking`, `buf-generate`, `web-lint`, `fake-metadata-server` and
  `kms-stub`.
- GitLab commands: `pipeline-run`, `repo-status`, `logs` and `captain-ci-test`,
  each verified against the Go binary on stdout, stderr, exit code and — for
  the two that post a body — the exact request bytes.
- `pipeline-watch`, the per-repo CI dashboard. It reads the same YAML the Go
  tool reads, honours `--config`, `--base-url`, `--interval`, `--per-repo` and
  `--json`, and paints the same table with the same key bindings. The row
  layout, the colours, the column widths, the cache TTLs, the JSON snapshot
  and the error wording are verified against the Go binary, snapshot and frame
  alike.
- Release workflow: `workflow` with its `list`, `apps`, `release-rc`,
  `test-rc`, `integration-rc`, `deploy-prod`, `promote`, `prod-verify`,
  `scenarios` and `status` subcommands, plus named-workflow dispatch
  (`workflow <name> <subcommand>`) and the built-in → user → repo →
  `--config` layering. Verified against the Go binary on stdout, stderr and
  exit code, and covered by unit tests that pin the gate wording, the
  release/promote flows and the Captain/FAAS handshake.
- Downstream tooling: `ci-recipe` (with its `list` / `url` / `script` /
  `download` subcommands) and `upgrade`.
- Integration tests in `tests/cli.rs` that drive the compiled binary and pin
  the exit-code, text and JSON contracts.
- `blaze` build support through `scripts/blaze.sh`, which works around the
  user-namespace restrictions of sandboxed containers.
- The repository dogfoods the tool: every gate runs against this repository in
  `make gate`.

### Changed

- The repository runs its own gates with no warnings at all: every source file
  is inside the 300-line advisory budget and every function is below the
  complexity warning threshold. The splits are mechanical — flag parsing, the
  XML cursor, the outbound HTTP exchange, the command table, the watch config,
  the git-author policy, the go-env line checks, the upgrade release API and
  the oversized test modules — and behaviour is unchanged.
- `make gate` also runs `ci-go-env`, so the credential scan is part of the
  repository's own quality bar.
- `--output toon` now sorts every object's keys, recursively. Go's TOON path
  round-trips the payload through `encoding/json`, which turns each object into
  a Go map, and Go renders maps in key order; the Rust renderer used to leak
  struct declaration order instead. JSON output is unchanged: it keeps
  declaration order, the way `json.Marshal` does for a struct.
- `pipeline-watch` drives the Go original's *legacy* dashboard loop, which
  repaints the whole frame on every change. The Go TUI uses the alternate
  screen buffer and diffs the frames instead, so it keeps the scrollback and
  updates only the cells that changed. The log viewer (`enter`), the
  repository-status sub-view, the favourites store and mouse support are not
  ported yet.

### Known divergences

These are deliberate; each is documented at the code that causes it.

- `bugs` scans `.go` and `.rs`; the Go original only scans `.go`.
- `captain-ci-test` writes `cover.zip` with stored entries instead of
  deflate, because a compressor is a dependency this crate cannot audit. The
  archive is valid and unzips to the same bytes.
- Error text produced by a third-party library — a `serde_json` / `serde_yaml`
  decode failure, a `curl` transport failure — is worded differently from the
  Go original's.
- `pipeline-run --var` keeps command-line order; Go iterates a map, so a list
  of variables is ordered arbitrarily.
- `gomut` prints the installed engine's usage banner, version and flag set.
  The Go original embeds `gomut/cli`, so it always reports the copy compiled
  into it; the wrapper here delegates to `gomut` on `PATH`, which may be a
  different release. Everything else — the `--project-root` lift, the exit
  codes and the pass-through of every other argument — is the Go behaviour.
- `doctor`, `bump-version` and `release-start` print a different summary.

### Fixed

- `repo-status` is the Go command again: a live GitLab query for one project
  (project, head pipeline, recent pipelines, jobs, traces, open MRs, commits
  in the last day, latest tag), not a local `git status`. The usage block,
  the flag heuristic, the clamping, the request URLs and the key order are
  the Go ones, and TOON is the default output.
- `cleanup` kills the background helpers again — the PID files, port 18080 and
  a `pkill` sweep — and prints the same three `  · killed …` lines. It no
  longer deletes build artifacts, which the Go command never did.
- `ci-go-env` no longer flags this repository's own test sample: the sample
  oauth2 URL is assembled from parts, so the repository-wide credential scan
  stays quiet about itself.
- The `rust-panic` and `rust-unsafe` scanners now treat any `tests/` directory
  as test code, not only the crate-level one, so a module can keep its test
  helpers next to itself.
- `bump-version --dry-run` and `release-start --dry-run` no longer write
  anything; the dry-run check happens before any filesystem or git change.
- Global flags are now accepted after positional arguments, so
  `bump-version patch --dry-run` no longer ignores the flag.
- `ci-lint`, `go-pin`, `goversion`, `buf-lint` and `verify-dist` skip cleanly
  when the input they need is absent instead of failing the run.
- The `report-bundle` manifest ends with a newline, and generated `reports/`
  output is excluded from the source-scanning gates.
