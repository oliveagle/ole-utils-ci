// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The general `workflow` usage block.
//!
//! `$CONFIG$` stands for the user config path; it is substituted at print
//! time so the constant keeps the Go original's single `%s` placeholder
//! without a format string (the block itself contains `{`/`}` in its YAML
//! example).

/// The `general` usage block.
pub const GENERAL: &str = r#"devine-utils-ci workflow — codify the binary release + test flow.

Every workflow has a NAME. The topology is DATA kept with the repo:
a repo-committed .devine/workflow.yaml (walked up from cwd to the git
root) IS the source of truth and follows the repo's commits. The binary
ships a built-in FALLBACK ('devine-golang-mcp-app') so the command works
with no config file at all. Precedence, lowest -> highest: built-in ->
~/.devine/utils/workflow.yaml -> .devine/workflow.yaml -> --config PATH.
Any layer may override any workflow by name, including the built-in.

USAGE
    devine-utils-ci workflow list                             # all known workflows
    devine-utils-ci workflow <name> <subcommand> [flags]      # named workflow
    devine-utils-ci workflow <subcommand> [flags]             # default (mcp-app)

SUBCOMMANDS
    list              enumerate every named workflow (built-in + user-declared)
    apps              list the test FAAS apps catalog
    release-rc        bump rc on binary repo main + push main+tag → verify UAT only
    test-rc           trigger the dev-test app pipeline + poll /version
    integration-rc    trigger integration app(s) + optional /version check
    deploy-prod       publish latest main image to Captain PROD groups + auto-rollout + spec verify
                      [--app <catalog-name|role|FAAS repo path>]  non-catalog apps are
                      resolved from FAAS and MUST be devine-golang (GolangDevine* code)
    promote           bump rc → stable on binary repo main + push + verify PROD
                      [GATED: integration-01 must be on rc; AUTO-RUNS integration-02 on success]
    prod-verify       verify a STABLE release end-to-end: prod artifactory present +
                      trigger each app's main pipeline (--wait) + /version == stable
                      [stable-only; the ONE place 生产发布验证 lives — consumers delegate here]
    status            one-shot snapshot: VERSION, tags, artifactory, test apps

FLAGS (common)
    --config <path>   explicit workflow YAML config (highest precedence)
    --dry-run         show what would happen, do not push/trigger
    --output <fmt>    text (default) | json
    --verbose         verbose runner output

CONFIG
    Topology follows the repo's commits. Precedence (lowest -> highest):
      1. built-in fallback   devine-golang-mcp-app (DefaultWorkflowConfig)
      2. user config         $CONFIG$
      3. repo config         .devine/workflow.yaml (walked up from cwd
                             to the git root)
      4. explicit            --config <path>

    Any layer may add a workflow or override an existing one by name,
    including the built-in devine-golang-mcp-app. Overriding the built-in
    from a file prints a notice to stderr. Example repo config:

        workflows:
          devine-golang-mcp-app:        # override the built-in fallback
            artifactory: { min_size_b: 2000000 }
          my-app:                        # a brand new workflow
            binary_repo: { name: my-app, project: ns/my-app, version_file: VERSION }
            artifactory: { uat_path: ..., prod_path: ... }
            test_apps:
              - name: my-dev-test
                project: faas/my-dev-test
                role: dev-test
                version_url: http://my-dev-test.fws.faas.qa.nt.ctripcorp.com/version

EXAMPLES
    # Which workflows exist?
    devine-utils-ci workflow list

    # mcp-app (default): show its 3 test apps
    devine-utils-ci workflow apps
    devine-utils-ci workflow devine-golang-mcp-app apps

    # One-click rc release + smoke test on the dev-test app
    devine-utils-ci workflow release-rc
    devine-utils-ci workflow test-rc --wait --poll-version

    # Drive integration-01 (gate app) then promote (gated)
    devine-utils-ci workflow integration-rc --app devine-app-integration-01-function --wait --poll-version
    devine-utils-ci workflow promote

    # Any OTHER named workflow works identically
    devine-utils-ci workflow my-app status

RELEASE RULES (enforced by 'workflow promote' on every workflow that has
integration-01 / integration-02 test apps)
    1. integration-01 (/version) must report the rc BEFORE promote runs
    2. promote auto-triggers integration-02 (/version) AFTER stable is live
"#;
