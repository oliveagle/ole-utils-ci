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

Installing the package installs the language-scoped binaries as well:
`ole-utils-ci` (the whole tool under the repository's name) plus one binary
per language group — `ole-utils-golang`, `ole-utils-rust`,
`ole-utils-tengo`, `ole-utils-proto`, `ole-utils-web` and
`ole-utils-common`. See [Language groups](#language-groups).


## Quick start

```sh
# The three checks that catch most problems
devine-utils-ci bugs
devine-utils-ci sanity
devine-utils-ci complexity

# Everything a pull request must pass
devine-utils-ci all

# The Rust gates only
devine-utils-ci rust all

# Structured output for CI annotations
devine-utils-ci bugs --output json
devine-utils-ci all --output toon
```

## The two environments

The tool runs in one of two environments, and `--mode` says which:

| | `--mode dev` | `--mode ci` |
|---|---|---|
| what it is | the developer's loop | the judge |
| scope | the crates the branch touched since `--base-branch` | the whole workspace |
| Rust driver | `blaze` when it is installed, else `cargo` | always `cargo` |
| gates | `rust-doc`, `rust-audit` and `coverage` are skipped, and say so | every gate |
| verdict | a smoke test, never a verdict | the verdict |

```sh
devine-utils-ci all --mode dev    # fast: the changed crates, the slow gates out
devine-utils-ci all --mode ci     # full: the whole workspace, every gate
```

`ci` can do everything `dev` can and more, so a pipeline may run either. A
`dev` run never pretends to be a `ci` run: each gate it leaves out reports
`skipped — <reason>`.

### A CI job cannot be degraded

Inside a CI system (`CI`, `GITHUB_ACTIONS`, `GITLAB_CI`, `BUILDKITE`) the
tool refuses the ways a job could be made weaker:

* `--mode dev` is refused unless the pipeline declared it on purpose with
  `DEVINE_UTILS_REQUIRE_MODE=dev`;
* `DEVINE_UTILS_RUST_DRIVER=blaze` is refused in `ci` mode — a fast driver
  answers from its own cache and can leave a test or a lint out;
* `DEVINE_UTILS_REQUIRE_MODE=ci` refuses any other mode, so a job that says
  it runs the full gate cannot be talked into running less.

The two pipelines of this repository are in
[`.github/workflows/ci.yml`](.github/workflows/ci.yml); every job pins its
mode there, and `tests/ci_workflow.rs` fails the build when a job stops doing
so.

## Commands

### Language groups

The commands below are flat: the language is part of the name
(`tengo-test`, `rust-clippy`, `go-pin`). Every command is also reachable
through the language group it belongs to, so a repository that mixes
languages can be driven one language at a time:

```sh
devine-utils-ci golang test        # same as `devine-utils-ci test`
devine-utils-ci rust clippy        # same as `devine-utils-ci rust-clippy`
devine-utils-ci tengo test         # same as `devine-utils-ci tengo-test`
devine-utils-ci proto breaking     # same as `devine-utils-ci buf-breaking`
devine-utils-ci common bugs        # same as `devine-utils-ci bugs`
devine-utils-ci common hook status # same as `devine-utils-ci hook status`

devine-utils-ci golang all         # every Go gate, in order
```

| Group | Owns |
|-------|------|
| `golang` | `install`, `lint`, `test`, `build`, `coverage`, `branch-coverage`, `test-report`, `gomut`, `version-check`, `go-pin`, `no-shell-exec`, `plugin-freshness`, `goversion`, `package-size` |
| `rust` | `rust`, `rust-fmt`, `rust-clippy`, `rust-test`, `rust-build`, `rust-doc`, `rust-audit`, `rust-manifest`, `rust-unsafe`, `rust-panic`, `rust-msrv` |
| `tengo` | `tengo-test`, `tengo-coverage`, `tengo-mutate` |
| `proto` | `buf-lint`, `buf-breaking`, `buf-generate` |
| `web` | `web-lint` |
| `common` | `bugs`, `sanity`, `license`, `structure`, `file-length`, `complexity`, `arch`, `artifact-size`, `hook` |

Every group has a standalone binary as well, so the group can be run without
typing the group name:

```sh
ole-utils-golang test               # same as `devine-utils-ci golang test`
ole-utils-rust all                  # every Rust gate, in order
ole-utils-tengo --help              # the commands of the group
```

A group runs nothing until you name a command: on its own it prints
the commands it has and exits. `all` runs every gate of the group, in
order, and stops at the first failure, exactly like `all` does. The flat
commands keep working unchanged.

### Git hooks

`hook` answers the questions that are cheap to fix before a commit exists and
expensive to fix once it is pushed:

```sh
devine-utils-ci hook install      # pre-commit, commit-msg and pre-push
devine-utils-ci hook status       # what is installed, and whether it is ours
devine-utils-ci hook uninstall    # remove only the hooks we installed
```

| Hook | Refuses |
|------|---------|
| `pre-commit` | a commit of more than **20 files**; a staged file over **5 MiB**; `.env`, `*.pem`, `*.key`, `*.p12`, `id_rsa`, `credentials.json`; a whitespace error or conflict marker in the staged diff |
| `commit-msg` | a message that is not `<type>(<scope>): <description>`, where `type` is one of `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`; a subject over 100 characters |
| `pre-push` | a push to `main` or `master` — open a merge request instead; a branch whose name is not `<type>/<slug>`, e.g. `fix/ci-hook` |

The numbers come from where review and history actually break down rather than
from round figures:

* **20 files per commit.** Past that a single `git show --stat` stops being one
  idea, and the reviewer switches from "is this correct" to "did anything look
  broken". Atomic commits — including one that renames a module boundary —
  stay well under it. A merge commit is exempt: it carries whatever the merged
  branch changed.
* **5 MiB per file.** Git never forgets a blob, so one large file costs every
  clone from then on. Source files are orders of magnitude smaller; a fixture
  that needs more room is a decision, not an accident.
* **`<type>/<slug>` branches.** The name says what the branch carries, matches
  the commit types, and gives the protected-branch rule something to point at:
  `main` is where work lands, never where it starts.

Every budget is overridable, flag first, then environment, then the default:

```sh
devine-utils-ci hook pre-commit --max-files 40            # a deliberate refactor
devine-utils-ci hook pre-commit --max-file-bytes 20m      # a fixture, on purpose
devine-utils-ci hook pre-push --protected main,release    # protect one more branch
HOOK_MAX_FILES=10 devine-utils-ci hook pre-commit         # same, from the environment
```

Committing on `main` is deliberately still allowed: the release flow commits
version bumps there. The gate is the push, the moment a change becomes
somebody else's problem.

`hook install` writes a three-line shell shim per hook that `exec`s the binary
by absolute path, so the policy lives in one place and keeps working in a shell
that does not have the tool on `PATH`. A hook file that we did not write is
never replaced without `--force` and never deleted at all.

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
| `--mode <dev\|ci>` | `ci` | The environment. `dev` is the fast loop: the changed crates since `--base-branch` under cargo, `blaze` when it is installed, `rust-doc`/`rust-audit`/`coverage` skipped. `ci` is the judge: the whole workspace through cargo, every gate. `release` is the old name of `ci` and still selects the full run. |
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
make gate-fast      # the dev loop: changed crates, blaze, slow gates skipped
make gate           # the full run: whole workspace, cargo, every gate
make blaze-build    # build with blaze
make blaze-test     # run the tests with blaze
```

`gate-fast` and `gate` are the two environments the tool now distinguishes, so
two CI pipelines can be just those two commands: a quick one on every push
(`make gate-fast`) and a full regression one before a merge (`make gate`). A
`gate-fast` pass is a smoke test, not a verdict — it prints the gates it left
out, and the Rust driver follows the environment, so `ci` never answers from a
fast driver's cache.

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

`deny.toml` is the dependency policy: the licences the tree may ship under,
the crates that are allowed to appear twice, and the advisories that are
ignored — each with its reason. The `rust-audit` gate runs `cargo deny check`
against it, so the policy is part of the gate rather than a document that
drifts away from it. The advisory list is empty: every crate the binary ships
is maintained.


## Licence

Apache-2.0. See [LICENSE](LICENSE).
