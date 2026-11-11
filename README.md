# devine-utils-ci

`devine-utils-ci` is the unified CI quality-gate tool. It bundles the static
checks a repository needs before code lands on `main` — formatting, linting,
complexity, file-length budgets, licence headers, dependency hygiene, coverage
thresholds — behind one binary and one structured output contract.

This repository is the **Rust rewrite** of the Go tool that shipped inside
[`devine_utils`](https://github.com/oliveagle/devine_utils). The command
surface, the `--output text|json|toon` contract and the exit-code semantics are
unchanged, so existing CI pipelines keep working. On top of the Go parity set
it adds a family of **Rust-specific gates** (`rust`, `rust-fmt`, `rust-clippy`,
`rust-test`, `rust-build`, `rust-doc`, `rust-audit`, `rust-manifest`,
`rust-unsafe`, `rust-panic`, `rust-msrv`).

## Installation

```sh
# From source
cargo install --path .

# With blaze (faster incremental builds)
scripts/blaze.sh install
```

The release profile is tuned for shipping: thin LTO, one codegen unit,
`panic = "abort"` and stripped symbols.

## Quick start

```sh
# The three checks that catch most problems
devine-utils-ci bugs
devine-utils-ci sanity
devine-utils-ci complexity

# Everything a pull request must pass
devine-utils-ci all

# The Rust gates only
devine-utils-ci rust

# Structured output for CI annotations
devine-utils-ci bugs --output json
devine-utils-ci all --output toon
```

## Commands

### Quality gates

| Command | What it does |
|---------|--------------|
| `bugs` | Scans for `BUG` / `FIXME` / `HACK` / `XXX` markers. Zero tolerance. |
| `sanity` | Trailing whitespace, conflict markers, missing final newlines. |
| `license` | Every source file must carry a licence header. |
| `structure` | Repository root layout and build-artifact hygiene. |
| `file-length` | Type-aware line budgets (source/script/html/declarative). |
| `complexity` | Cyclomatic complexity per function. |
| `package-size` | Warning-only package growth budgets. |
| `artifact-size` | Release-artifact size budgets. |
| `go-pin` | Go toolchain pin drift. |
| `goversion` | `go.mod` versus the installed Go toolchain. |
| `no-shell-exec` | Production code must not shell out to `bash`/`sh`. |
| `plugin-freshness` | Plugin `.so` files must be newer than their sources. |

### Rust gates

| Command | What it does |
|---------|--------------|
| `rust` | Every Rust gate below, in order. |
| `rust-fmt` | `cargo fmt --check`. |
| `rust-clippy` | `cargo clippy --all-targets -- -D warnings`. |
| `rust-test` | `cargo test`. |
| `rust-build` | `cargo build --all-targets`. |
| `rust-doc` | `cargo doc --no-deps`. |
| `rust-audit` | Dependency advisories and licence audit. |
| `rust-manifest` | `Cargo.toml` hygiene: wildcards, unpinned git deps, release profile. |
| `rust-unsafe` | No `unsafe` blocks in production code. |
| `rust-panic` | No `unwrap` / `expect` / `panic!` in production code. |
| `rust-msrv` | The declared `rust-version` must be a supported release. |

### Toolchain

| Command | What it does |
|---------|--------------|
| `install` | Fetch the dependencies (`go mod vendor`). |
| `lint` | `gofmt -s`, `go mod edit -fmt`, `go vet`, `golangci-lint`. |
| `test` | `go test` with the race detector. |
| `build` | `go build ./...`. |
| `coverage` | Line coverage against the threshold. |
| `branch-coverage` | Branch coverage (gobco). |
| `gomut` | Go mutation testing through the installed `gomut` engine. |
| `tengo-test` | Tengo tool unit tests with fake `call_tool` mocks. |
| `tengo-coverage` | Tengo line + branch coverage gates. |
| `tengo-mutate` | Tengo source mutation testing. |
| `test-report` | JUnit XML report. |
| `all` | The whole quality gate, in CI order. |

### Version and release

| Command | What it does |
|---------|--------------|
| `version-check` | `VERSION`, every declared version mirror and the git tag must agree. |
| `bump-version` | Bump the version: `patch` \| `minor` \| `major` \| `rc` \| `promote`. |
| `release-check` | The gate that runs before a release is cut. |
| `release` | Create the GitLab release for the current tag. |
| `release-start` | Create the release branch and the bump commit. |
| `release-finish` | Tag the merged release and push the tag. |
| `release-model` | Explain the simple and strict release models. |
| `protect-set` | Set branch and tag protections for a project. |

### Repository

| Command | What it does |
|---------|--------------|
| `doctor` | Run diagnostic checks on the toolchain, the environment and the project layout. |
| `git-author` | Enforce the per-repo commit identity policy. |
| `cleanup` | Stop the background helpers (`fake-metadata-server`) and release their ports. Call it from `after_script` or a `trap`. |
| `report-bundle` | Collect the reports of a CI run into one directory. |
| `verify-dist` | Verify a `dist/` directory is a valid SPA production build (Vite/Vue/React). |

### More checks

| Command | What it does |
|---------|--------------|
| `ci-lint`, `glab-ci-lint` | Validate the GitLab CI pipeline file; errors when there is none, the way Go does. |
| `ci-go-env` | Check Go env and proxy config in the build scripts. |
| `arch` | Dependency cycles and layer direction. |
| `buf-lint` | The proto naming conventions. |
| `buf-breaking` | Check the protos for breaking changes. |
| `buf-generate` | Generate code from the proto files. |
| `web-lint` | Syntax sanity for the front-end sources. |
| `fake-metadata-server` | Stub the Ctrip metadata endpoint for tests. |
| `kms-stub` | Serve a DEV ONLY local Ctrip KMS stand-in. |

### GitLab

| Command | What it does |
|---------|--------------|
| `pipeline-run` | Trigger a GitLab pipeline for a project (needs `GITLAB_TOKEN`). |
| `repo-status` | Live pipeline status of one project, for AI agents (no cache; default `-o toon`). |
| `pipeline-watch` | The per-repo CI dashboard: one row per watched project. |
| `logs` | Query or tail Clog logs (needs `OSG_CLOG_TOKEN`). |
| `captain-ci-test` | Reproduce the Captain-CI Test job artifacts. |

`repo-status` answers "is my last push green?" for one project without going
through the dashboard: it reads the same config file as `pipeline-watch`,
queries GitLab live (no cache) and prints one object — project, head pipeline,
recent pipelines, open MRs with their head-pipeline status, commits in the last
day and the latest release tag. It takes `-config`, `-base-url`, `-pipelines`,
`-pipeline`, `--jobs`, `--trace`, `--mrs`, `--list-repos`, `-timeout` and
`--output toon|json|text`. TOON is the default because the command exists for
AI agents; `--pretty` is the pre-TOON alias for JSON, and `--list-repos` prints
the configured projects and exits without touching the network.

`pipeline-watch` reads the same YAML the Go tool reads — `base_url`, `token`,
`refresh.interval_seconds`, `per_repo`, `local_root` and a `repos` list of
`group:` / `project:` entries — and takes `--config`, `--base-url`,
`--interval`, `--per-repo` and `--json`. `--json` prints one snapshot and
exits; without it the command paints the dashboard and reads keys until `q`.
The key set is the Go one: `q` quits, `r` refreshes, `+` / `-` move the
refresh interval, `j` / `k` move the selection, `o` opens the selected
pipeline in the browser.

Three parts of the Go command are not in the Rust port yet:

- the bubbletea TUI. The Rust port drives the Go original's *legacy* loop,
  which repaints the whole frame on every change. The Go TUI uses the
  alternate screen buffer and diffs the frames instead, so it keeps the
  scrollback and updates only the cells that changed.
- the log viewer (`enter`), the repository-status sub-view and the favourites
  store.
- mouse support.

### Who parses the flags

Most commands share one flag set: the global one, with `--output`,
`--project-root`, `--verbose` and the rest. A few commands own a *private*
flag set instead, exactly as the Go original does, so their flags never reach
the shared parser and a bad flag prints their own usage block:

| Command | Flag set | Bad flag |
|---------|----------|----------|
| `go-pin`, `no-shell-exec`, `plugin-freshness`, `web-lint` | private | exits 2 |
| `report-bundle` | private | exits 1 |
| `gomut`, `tengo-*`, `logs`, `pipeline-run`, `pipeline-watch`, `workflow`, `upgrade`, `kms-stub`, `ci-recipe` | private | exits 1 |
| `verify-dist`, `cleanup` | none (raw arguments) | — |
| everything else | the global set | exits 1 |

The four focused lint gates also default `--output` to `toon` rather than
`text`, the way Go's `focusedOutputMode` does, because they exist to be read
by an agent. `web-lint -h` prints the `flag` package's block on stderr and
the hand-written one on stdout, precisely as the Go command does.
`buf-breaking` lifts `--against` out before the shared parser sees it, so the
baseline Go's own `--help` advertises is accepted here even though Go's flag
set rejects it as undefined.

Everything else — the row layout, the colours, the column widths, the key
bindings, the cache TTLs, the JSON snapshot and the error wording — is
verified against the Go binary, snapshot and frame alike.

### Release workflow

`workflow` drives the binary release + test flow. The topology is DATA kept
with the repo: a repo-committed `.devine/workflow.yaml` (walked up from the
working directory to the git root) is the source of truth, and the binary
ships a built-in fallback so the command works with no config file at all.
Precedence, lowest to highest: built-in → `~/.devine/utils/workflow.yaml` →
`.devine/workflow.yaml` → `--config PATH`. Any layer may override any
workflow by name, including the built-in one.

| Command | What it does |
|---------|--------------|
| `workflow list` | Enumerate every named workflow (built-in + user-declared). |
| `workflow apps` | List the test FAAS apps catalog. |
| `workflow release-rc` | Bump an rc on the binary repo `main` + push `main`+tag → verify UAT only. |
| `workflow test-rc` | Trigger the dev-test app pipeline + poll `/version`. |
| `workflow integration-rc` | Trigger the integration app(s) + optional `/version` check. |
| `workflow deploy-prod` | Publish the latest main image to the Captain PROD groups + auto-rollout + spec verify. |
| `workflow promote` | Bump rc → stable on the binary repo `main` + push + verify PROD. Gated on integration-01. |
| `workflow prod-verify` | Verify a STABLE release end to end: PROD artifact + each app's `/version`. |
| `workflow scenarios` | Run the MCP scenario matrix against the integration apps. |
| `workflow status` | One-shot snapshot: VERSION, tags, artifactory, test apps. |

`workflow <name> <subcommand>` runs a named workflow; `workflow <subcommand>`
runs the default one (`devine-golang-mcp-app`). Every subcommand takes
`--config <path>`, `--dry-run`, `--output text|json` and `--verbose`.

### Downstream tooling

| Command | What it does |
|---------|--------------|
| `ci-recipe` | Render or download the downstream CI artifact recipe. |
| `upgrade` | Download and install a specific version from GitLab releases. |

### Porting status

The whole Go command surface is implemented here. The parts of `pipeline-watch`
that are still missing are listed above.

Every command accepts `--output json` or `--output toon` where it reports
findings, and every command exits `0` on success, `1` on a failed check or an
unknown flag, and `1` for an unknown subcommand.

## Global flags

| Flag | Default | Meaning |
|------|---------|---------|
| `--project-root <path>` | `.` | Repository to inspect; resolved to an absolute path. |
| `--mode <dev\|release>` | `release` | `dev` runs only the packages changed since `--base-branch`. |
| `--base-branch <name>` | `main` | Diff base for the incremental mode. |
| `--coverage-threshold <n>` | `80` | Line-coverage floor (%). |
| `--branch-coverage-threshold <n>` | `80` | Branch-coverage floor (%). |
| `--coverage-allowlist <pkg,…>` | (empty) | Packages allowed to sit below the threshold. |
| `--covermode <set\|atomic\|count>` | `set` | `go test -covermode` value. |
| `--max-file-lines <n>` | `500` | Source-file line ceiling. |
| `--complexity-warn <n>` | `15` | Complexity warning threshold. |
| `--max-complexity <n>` | `20` | Complexity hard ceiling. |
| `--build-tags <tags>` | (empty) | Build tags for the Go toolchain. |
| `--parallel <n>` | `0` | `go test -parallel N`; `0` means `GOMAXPROCS`. |
| `--junit <path>` | `junit.xml` | JUnit XML output path. |
| `--coverage-html <path>` | `coverage.html` | HTML coverage report path. |
| `--coverage-json <path>` | `coverage.json` | JSON coverage report path. |
| `--output <text\|json\|toon>` | `text` | Structured output format. |
| `--verbose`, `-v` | `false` | Print the underlying tool output. |
| `--dry-run`, `-n` | `false` | Change nothing. |
| `--ts`, `--timestamp` | `false` | Prefix every output line with a timestamp. |

Thresholds may only be **tightened**. Asking for a larger budget than the
built-in hard ceiling is a configuration error, not a warning.

## Output contract

`--output json` and `--output toon` share one schema, so a pipeline can switch
formats without changing its parser:

```json
{
  "command": "bugs",
  "ok": false,
  "summary": "bugs: 1 marker(s) found in 1 file(s)",
  "counts": { "error": 1, "warning": 0, "info": 0 },
  "diagnostics": [
    {
      "path": "src/main.rs",
      "line": 12,
      "col": 0,
      "rule": "bugs",
      "severity": "error",
      "detail": "marker BUG"
    }
  ]
}
```

`counts` is always present. `diagnostics` and `extra` are omitted when empty.
A diagnostic omits `path`, `line`, `col`, `rule` and `detail` when they carry
no information.

TOON output is produced from the same JSON value, so both formats expose
exactly the same keys.

## Project configuration

`.devine-utils-ci.jsonc` tightens the built-in budgets for one repository:

```jsonc
{
  "ignore": {
    "paths": ["vendor/**", "third_party/**"]
  },
  "file-length": {
    "profiles": { "source": { "warn": 300, "max": 400 } }
  },
  "complexity": { "warn": 12, "max": 18 }
}
```

Unknown fields, raised budgets and profile switches are configuration errors.

## Repository layout

```
src/
├── main.rs           # CLI dispatch and usage text
├── lib.rs            # Module tree and version
├── cli.rs            # Hand-written, Go-flag-compatible argument parser
├── config.rs         # Global flags, defaults and hard budgets
├── quality_config.rs # .devine-utils-ci.jsonc parsing and glob matching
├── output.rs         # The text/json/toon report contract
├── fsutil.rs         # File walking and generated-file detection
├── runner.rs         # External command execution with a timeout
├── git.rs            # Read-only git plumbing for status/release commands
├── version.rs        # Version parsing and the bump arithmetic
├── static_regex.rs   # The one place a `Regex` may be built statically
├── process.rs        # Go's wording for a child process that died badly
├── gates/            # The language-agnostic quality gates
├── rustgate/         # The Rust-specific gates (toolchain, scanner, source)
├── commands/         # Subcommand wiring, one module per command family
└── tests/cli.rs      # End-to-end tests that drive the compiled binary
```

## Development

```sh
make build          # cargo build --all-targets
make test           # cargo test --all-targets
make lint           # fmt + clippy
make gate           # run the tool against this repository
make blaze-build    # build with blaze
make blaze-test     # run the tests with blaze
```

The repository dogfoods the tool: `make gate` runs every gate against this
repository and must stay green — including `ci-go-env`, which scans the tree
for likely credentials. In particular the source budget is 300/450 lines and
the complexity budget is 12/18 (see `.devine-utils-ci.jsonc`), both tighter
than the built-in defaults; the tree currently sits inside both warning
thresholds, so a `make gate` run reports no warnings at all.

`scripts/blaze.sh` wraps `blaze` and restarts its daemon with
`BLAZE_SANDBOX=off`, because container images that block
`unshare(CLONE_NEWUSER)` otherwise fail every build with
`/proc/self/setgroups: Permission denied`.

## Licence

Apache-2.0. See [LICENSE](LICENSE).
