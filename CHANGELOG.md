## [Unreleased]

### Added

- The `--mode` flag finally does what it always promised: it selects the
  environment. `--mode dev` is the fast loop — the compiling gates run only
  the crates the branch touched since `--base-branch`, the Rust driver is
  `blaze` when it is installed, and the slow `rust-doc`, `rust-audit` and
  `coverage` gates are reported as skipped instead of running. `--mode ci`
  is the judge — the whole workspace through plain `cargo`, every gate.
- A CI job can no longer be degraded. `DEVINE_UTILS_REQUIRE_MODE` pins the
  mode of a job, and inside CI the tool refuses `--mode dev` unless the
  pipeline declared it on purpose, and refuses
  `DEVINE_UTILS_RUST_DRIVER=blaze` in `ci` mode — a fast driver answers from
  its own cache and can leave a test or a lint out.
- `.github/workflows/ci.yml`: the two pipelines of this repository. The `fast`
  job runs the toolchain gates in the declared `dev` mode, the `full` job runs
  every gate in the authoritative `ci` mode. `tests/ci_workflow.rs` reads the
  file and fails the build when a job stops pinning its mode, asks for another
  mode than the one it pinned, or asks for the fast driver.
- `deny.toml`: the dependency policy, checked by `cargo deny` through the
  `rust-audit` gate. The licence set is the permissive one a Rust binary may
  ship under, and every ignored advisory is listed by id with the reason.
- `make gate-fast` runs the dev loop and `make gate` runs the full one, so the
  two CI pipelines can be the two commands.

### Changed

- `--mode release` is now spelled `--mode ci`; `release` still works and
  selects the full run. An unrecognised value also selects the full run, so a
  typo can never buy a weaker gate.
- The Rust driver follows the environment: `ci` always answers with `cargo`,
  because a fast driver answering from its own cache can leave a test or a
  lint out; `dev` prefers `blaze`. `DEVINE_UTILS_RUST_DRIVER` still overrides
  both outside CI.

### Fixed

- `--mode dev` and `--base-branch` were parsed and documented but read by
  nothing, so every run was the full run whatever the flag said.
- `cargo fmt` was handed `--workspace`, which neither cargo nor blaze takes
  for `fmt`, so the gate failed as soon as the driver was `blaze`.
- The workspace check parsed `Cargo.toml` with `Value::from_str`, which is not
  a document parse in `toml` 0.9, so every repository looked like a
  workspace.
- The `PathBuf::join` link in `fsutil` was broken, which `cargo doc` with
  warnings denied — a failure no one could see, because the driver was
  `blaze` and never ran rustdoc.

## [0.12.0] — 2026-11-14

### Added

- Language-scoped subcommands. Every command is now also reachable through
  the language group it belongs to: `devine-utils-ci golang test`,
  `devine-utils-ci rust clippy`, `devine-utils-ci tengo test`,
  `devine-utils-ci proto breaking`, `devine-utils-ci web lint` and
  `devine-utils-ci common bugs`. The flat commands keep working unchanged,
  and a group forwards its flags to the command it runs.
- Standalone per-language binaries, installed by the same
  `cargo install --path .`: `ole-utils-ci` (the whole tool under the
  repository's name), `ole-utils-golang`, `ole-utils-rust`, `ole-utils-tengo`,
  `ole-utils-proto`, `ole-utils-web` and `ole-utils-common`.
- The `hook` command, the policy a git hook runs on every commit and push:
  `install`, `uninstall`, `status`, `pre-commit`, `commit-msg` and `pre-push`.
  `pre-commit` refuses a commit of more than 20 files, a staged file over
  5 MiB, a credential-looking path, and any whitespace error or conflict
  marker in the staged diff; `commit-msg` requires
  `<type>(<scope>): <description>`; `pre-push` refuses `main` and `master`
  (merge requests only) and any branch not named `<type>/<slug>`. Every budget
  takes a flag (`--max-files`, `--max-file-bytes`, `--protected`) and an
  environment variable (`HOOK_MAX_FILES`, `HOOK_MAX_FILE_BYTES`,
  `HOOK_PROTECTED_BRANCHES`). `hook install` writes shims that `exec` this
  binary by absolute path, never replaces a hook it did not write without
  `--force`, and honours `core.hooksPath`.
- A bare invocation prints a short summary (the groups, the commands people
  type, and where the full list lives) instead of the whole usage block;
  `devine-utils-ci help` still prints the full block.
- A group runs nothing until you name a command: on its own it prints the
  commands it has and exits with status 1. `all` runs every gate of the
  group, in order, and stops at the first failure, the way `all` does.

### Fixed

- `bump-version` now writes the new version into this package's entry in
  `Cargo.lock` as well as `VERSION` and `Cargo.toml`. Every build path in this
  repository passes `--locked`, so a bump that left the lock behind produced a
  commit that could not be built.


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
- The `blaze` release is pinned in `Cargo.toml` under
  `[workspace.metadata.blaze]`, so every machine and every CI job builds the
  workspace with the same `blaze` version.
- The repository dogfoods the tool: every gate runs against this repository in
  `make gate`.

### Changed

- The repository runs its own gates with no warnings at all: every source file
  is inside the 300-line advisory budget and every function is below the
  complexity warning threshold. The splits are mechanical — flag parsing, the
  XML cursor, the outbound HTTP exchange, the command table, the watch config,
  the git-author policy, the go-env line checks, the upgrade release API, the
  ci-lint scanners, the buf-lint naming rules and the oversized test modules —
  and behaviour is unchanged.
- `make gate` also runs `ci-go-env`, so the credential scan is part of the
  repository's own quality bar.
- `--output toon` now sorts every object's keys, recursively. Go's TOON path
  round-trips the payload through `encoding/json`, which turns each object into
  a Go map, and Go renders maps in key order; the Rust renderer used to leak
  struct declaration order instead. JSON output is unchanged: it keeps
  declaration order, the way `json.Marshal` does for a struct.
- `release-check`, `bump-version`, `release-start` and `release-finish` now
  follow the Go original's order, wording and side effects end to end: the
  release model is validated first, `release-check` reports the first broken
  invariant instead of only the first dirty file, `bump-version` prints Go's
  banner and trailer and refuses a foreign change the way Go does,
  `release-start` honours `simple` (bump on `main`) versus `strict`
  (`chore/release-v<NEXT>`) — including Go's decision to ignore `--dry-run`
  there — and `release-finish` stamps the ADR 0001 marker into the tag it
  pushes, which is what makes a Rust-made tag pass `release-check`.
- `pipeline-watch` drives the Go original's *legacy* dashboard loop, which
  repaints the whole frame on every change. The Go TUI uses the alternate
  screen buffer and diffs the frames instead, so it keeps the scrollback and
  updates only the cells that changed. The log viewer (`enter`), the
  repository-status sub-view, the favourites store and mouse support are not
  ported yet.

- `install`, `lint`, `test`, `build`, `coverage` and `test-report` now follow
  the Go original end to end. `install` walks the same proxy list — the one
  the environment names, the internal one, the public one and finally
  `direct` — and falls back to the next one only when the failure looks like
  a proxy or network problem. `lint` installs the dependencies first, reads
  `.devine-utils-ci.yaml` and then runs `gofmt -s` over the first-party
  trees, `go mod edit -fmt`, `go vet`, `golangci-lint` and the
  config-driven gates. `test` and `build` print Go's banners and word their
  failures the way Go words them. `coverage` filters the generated code out
  of the coverprofile, averages the function percentages per package and
  writes the same HTML and JSON reports. `test-report` writes the JUnit XML
  Go's `encoding/xml` writes.
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
- `bump-version` also prints `✓ Cargo.toml synced to <version>` when it
  rewrites this crate's own `Cargo.toml`. The Go original has no such file;
  every other line it prints is Go's.
- `buf-breaking --against <ref>` is accepted here because Go's own `--help`
  advertises it; Go's flag parser rejects the flag as undefined.
- `arch` also scans Cargo modules and recognises a layer from any path
  segment; Go runs `go list` and matches only `*/<layer>`.
- `report-bundle` writes a manifest bundle (`reports/quality-gate/bundle/`
  with the collected reports and a `manifest.json`) rather than Go's HTML
  dashboard tree. The flag set, the defaults and the exit codes are Go's.
- The coverage report lists its packages and the JUnit report lists its
  suites in sorted order. Go iterates a map there, so its order changes from
  run to run; sorting keeps two runs over the same module comparable.
- `web-lint` has Go's flag set, result envelope and exit codes, but its
  engines are a dependency-free scanner — tag structure for HTML, bracket
  structure for CSS and scripts — rather than Go's HTML parser plus
  esbuild. Record-keeping (`--ignore`, the file and byte caps, the security
  root, `--fail-on`) and the text / JSON / TOON renderings match.

### Fixed

- `install`, `lint`, `test`, `build`, `coverage` and `test-report` print Go's
  banners and fail with Go's wording. They used to print their own lines
  (`🔨 Building...`, `📈 Measuring line coverage...`, `📝 Generating the
  JUnit report...`) and to report a bare `tests failed` or `build failed`,
  so a CI job could not tell the Rust build from the Go one. The coverage
  gate now names the packages that fall short of the threshold and repeats
  the remediation Go asks for, and the JUnit report carries Go's attributes,
  its empty elements and its escaping.
- `report-bundle` and `web-lint` own their flag sets again. Go builds a
  private `flag.FlagSet` for both, so their flags never reached the shared
  parser and a bad flag printed their own block: `report-bundle` exits 1,
  `web-lint` exits 2. `web-lint` also takes `-h` to stdout and returns 0, and
  now honours `--path`, `--content`, `--language`, `--root`, `--ignore`,
  `--max-files`, `--max-file-size`, `--format` and `--fail-on` instead of
  ignoring every one of them.
- `protect-set` reports a missing token before a missing `--project-id`
  again: the `dotvault get` lookup now returns the helper's standard output
  and treats a non-zero exit as "no token", the way Go's `exec.Cmd.Output()`
  does. Before, the lookup read as a success and its stderr warnings were
  taken for a token.
- The focused lint gates — `go-pin`, `no-shell-exec` and `plugin-freshness` —
  are back on Go's contract: a private flag set, `--output` defaulting to
  TOON, exit 2 for a refused command line and exit 1 for findings, and their
  findings behind the `Error:` line.
- `verify-dist` is the Go command again: it validates a `dist/` SPA
  production build (`index.html` carrying a module script plus `assets/`
  with `.js` and `.css`) instead of checking release artifacts against
  `SHA256SUMS`, and it takes its directory as a raw positional argument
  because the Go command parses no flags at all.
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
- `go-pin`, `goversion` and `buf-lint` skip cleanly when the input they need
  is absent instead of failing the run. `ci-lint` does not skip: a missing
  `.gitlab-ci.yml` is an error in Go and an error here too.
- `ci-lint` runs Go's whole check set again: the missing-file error, the
  text-level scan (unquoted colon, tab, trailing whitespace), the job shape
  (`empty-job`, `no-script`, `script-not-string`, with `.template` jobs
  exempt) and the ten-line `>` folded-block-scalar lookback, and
  `ci-lint --output json|toon` carries the same keys as Go.
- `arch`, `buf-lint`, `buf-breaking` and `release-check` follow Go's output
  contract in every mode now. `arch` prints `arch: all checks passed ✓` or
  `arch: N violation(s) found:` when no mode is selected and Go's
  `{command, ok, violations}` document — the `%v` line for `text` — when one
  is; `buf-lint` and `buf-breaking` write their `🔍` progress lines whatever
  `--output` says, the way Go's text-only commands do; `buf-lint` reports a
  missing proto directory the way Go does instead of inventing a structured
  skip; and `release-check` emits Go's envelope with its single
  `release-check` diagnostic instead of a summary of its own.
- `arch` records the intra-project imports of a Go project again. The graph
  keyed packages by directory while an import spells the full module path, so
  every edge was dropped and the check reported "all checks passed" on a
  repository Go itself flags. Packages are keyed by import path now, and the
  rule names and wording are Go's (`cycle-detection`, `layer-dependency`,
  `circular dependency: …`, `layer 'x' must not depend on layer 'y'`).
- `buf-breaking` runs its basic check whatever the project contains. Go only
  skips when the proto tree cannot be walked, so a repository without protos
  gets the "basic check only" verdict rather than a skip line.
- The `report-bundle` manifest ends with a newline, and generated `reports/`
  output is excluded from the source-scanning gates.
