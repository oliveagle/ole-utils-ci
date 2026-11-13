// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `Commands:` block of the usage text.
//!
//! It is a verbatim copy of the Go original's `printUsage`, split out
//! only to keep both files inside the line budget. Nothing here is
//! rewritten: the descriptions are what users read before they pick a
//! subcommand.

/// The command list, from `Commands:` up to (not including) `Global Flags:`.
pub const COMMANDS: &str = r#"Commands:
  install              Install dependencies (go mod vendor).
                       Retries with multiple proxy fallbacks on network errors.
                       Auto-sets GONOSUMCHECK to bypass go.sum checksum
                       verification for internal GitLab modules.
  lint                 Run the composite lint gate: gofmt -s, go mod edit -fmt,
                       go vet, golangci-lint, plus configured Go-pin,
                       no-shell, plugin freshness and web syntax checks.
  go-pin               Check the .go-version toolchain pin across go.mod,
                       GitLab CI, Dockerfile and require-go.sh.
  no-shell-exec        Reject production Go code that executes bash/sh.
  plugin-freshness     Reject plugin .so files older than their source.
  web-lint             Lint HTML/CSS/JS/TS/TSX/JSX; accepts path or content.
  arch                 Run architecture rule checks (cycles, layers, naming).
  test                 Run tests with race detection.
                       Steps:
                         1. Install dependencies (go mod vendor via install)
                         2. go test -race -tags=<build-tags> -parallel N ./...
                         3. On failure: shows last 4KB of test output (actual failures)
                       Flags: --build-tags, --parallel, --verbose
                       Note: This command runs go test directly. Projects that
                       depend on external services at init-time (e.g. Ctrip
                       foundation metadata, QConfig, credis) must start
                       the required mock/fake servers BEFORE running this
                       command, or use --build-tags to exclude them.
  build                Build project (go build -o /dev/null ./...)
  coverage             Run coverage check (line coverage).
                       Steps:
                         1. Install dependencies (go mod vendor via install)
                         2. go test -coverprofile=coverage.out -covermode=<mode> ./...
                         3. Filter generated code (*.pb.go, "DO NOT EDIT") from profile
                         4. Parse per-package coverage and compare vs threshold
                       Flags: --coverage-threshold, --branch-coverage-threshold,
                               --covermode, --build-tags, --include-generated
  branch-coverage      Run branch coverage check (gobco)
  gomut                Run Go mutation testing with the embedded gomut engine
  tengo-test           Run Tengo tool unit tests with fake call_tool mocks
                       Flags: --src, --cases, --tool, --report, --junit,
                               --timeout, --output
  tengo-coverage       Run Tengo line + branch coverage gates
                       Flags: --src, --cases, --tool, --line-threshold,
                               --branch-threshold, --report, --lcov,
                               --timeout, --output
  tengo-mutate         Run Tengo source mutation testing
                       Flags: --src, --cases, --tool, --threshold,
                               --report, --timeout, --output
  test-report          Generate JUnit XML test report
  version-check        Check VERSION file consistency
  structure            Check directory structure
  bugs                 Scan for BUG/FIXME/HACK/XXX markers
  buf-lint             Run buf lint on proto files
  buf-breaking         Check for breaking proto changes
  buf-generate         Generate code from proto files
  file-length          Check type-aware source/template/config file budgets
                       (warn + hard limits; --max-file-lines is a global lower cap)
  complexity           Cyclomatic complexity for Go/Tengo/TS/JS
                       (warn default 15, hard max default 20)
  package-size         Warning-only Go package growth budgets
  artifact-size        Release-artifact budgets (default warn 50 / max 80 MiB)
  sanity               Check trailing whitespace, conflict markers
  license              Check license headers
  goversion            Check Go version vs go.mod requirement
  ci-go-env            Check Go env / proxy config in build scripts (GONOPROXY
                       forcing, hardcoded deploy tokens, missing anonymous
                       GOPROXY). Flags: --file=<path> (default .paas/build.sh),
                       --output=toon|json
  ci-lint              Validate .gitlab-ci.yml syntax (local static)
  glab-ci-lint         Validate .gitlab-ci.yml via GitLab /ci/lint (server, needs GITLAB_TOKEN)
  bump-version         Bump VERSION (patch/minor/major|rc|promote), commit, and tag
  release              Create GitLab release (CI_JOB_TOKEN via CI env vars)
  release-start        Initiate a new release (rc|promote|patch/minor/major; simple model)
  release-finish       Complete a release: create & push tag (--release-model simple|strict)
  release-check        Validate release consistency (VERSION, branch, tag, git state)
  release-model        Show release model documentation (single-main + rc pre-release)
  protect-set          Set branch/tag protections for a repo (--project-id, --release-model)
  ci-recipe            Render / download the downstream CI artifact recipe
  upgrade              Download and install a specific version of devine-utils-ci from GitLab releases
  captain-ci-test      Reproduce Captain-CI Test job (cover.xml/cover.zip/junit.xml)
  pipeline-watch       Interactive TUI dashboard: pipelines / MRs / commits across all watched repos
  pipeline-run         Trigger a GitLab pipeline for a project (needs GITLAB_TOKEN)
  workflow             Codify the binary release + test flow (release-rc / test-rc /
                       integration-rc / promote / status / apps). Enforces the
                       integration-01 gate before promote and auto-runs
                       integration-02 after promote.
  repo-status          Live per-repo pipeline status (for AI coding agents; no cache; default output=TOON)
  logs                 Query / tail Clog logs from OSG getClogPost endpoint (needs OSG_CLOG_TOKEN)
  git-author           Enforce per-repo git commit identity (e.g. company remotes must use <user>@trip.com)
  doctor               Run diagnostic checks on CI/dev environment.
                       Checks Go version, GOPROXY, GONOSUMCHECK,
                       METADATA_DOMAIN, vendor consistency, go.mod.
                       Exit code 1 if any critical check fails.
  cleanup              Kill all devine-utils-ci background processes
                       (fake-metadata-server, etc.) and release ports.
                       Call in after_script or trap.
  verify-dist          Verify a dist/ directory is a valid SPA production
                       build (Vite/Vue/React). Checks index.html has
                       <script type="module"> and assets/ has .js/.css.
                       Usage: devine-utils-ci verify-dist [path/to/dist]
                       Without path, defaults to ./dist.
                       Exit code 1 if dist is broken (prevents deploying
                       placeholder/white-screen builds via Go embed).
  fake-metadata-server Start a fake Ctrip metadata server in background
                       (no shell & needed). Serves empty metadata on
                       /api/ctrip/metadata.json so foundation.init()
                       doesn't panic. Run BEFORE test/coverage.
                         devine-utils-ci fake-metadata-server       # background (default)
                         devine-utils-ci fake-metadata-server --run # foreground (blocking)
                         export METADATA_DOMAIN=http://localhost:18080
                         devine-utils-ci cleanup                    # stop it
                       Env: FAKE_METADATA_PORT (default: 18080)
  kms-stub             Serve a DEV ONLY local Ctrip KMS stand-in
                       (/query-pwd + /query-key) for local runtime work.
                       Binds loopback only; refuses anything else.
                         devine-utils-ci kms-stub --listen 127.0.0.1:18080 \
                           --data ./kms-stub.yaml
                       Pair with a runtime built with -tags devkms:
                         KMS_ENDPOINT=http://127.0.0.1:18080 server serve
                       Stub data file is git-ignored; never use in a deploy.
  report-bundle        Aggregate every quality-gate check (structure/hygiene/complexity/CI/release) + coverage into a browsable reports/ tree (HTML + CSV)
  hook                 The commit and push policy checks a git hook runs:
                         install, uninstall, status, pre-commit, commit-msg,
                         pre-push. Defaults: 20 files/commit, 5 MiB/file,
                         main+master only through a merge request, branch
                         names of the <type>/<slug> form.
  all                  Run all checks (lint, test, build, coverage, etc.)
  version              Show version
  help                 Show this help

"#;
