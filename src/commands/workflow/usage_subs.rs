// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The per-subcommand usage blocks.
//!
//! These are the Go original's help texts, verbatim. Only the general block
//! is reachable from the command line today (`workflow -h`, and the two
//! "unknown subcommand" paths print it), but the per-subcommand blocks are
//! part of the command's documented surface and are covered by tests.

/// The `prod_verify` usage block.
pub const PROD_VERIFY: &str = r#"workflow prod-verify — production release verification (stable-only).

Runs immediately AFTER the (manual) Upload PROD on a stable tag. It is
the single authoritative home for 生产发布验证: a consumer repo (e.g.
devine-golang-mcp-app) MUST NOT trigger release pipelines itself — it
delegates to this command (devine_groups rule, 2026-09-28).

USAGE
    devine-utils-ci workflow prod-verify [<x.y.z>] [--from <x.y.z>]
                                         [--apps name,name] [--artifact-timeout N]
                                         [--wait-timeout N] [--poll-timeout N]
                                         [--skip-artifact] [--dry-run] [--verbose]

FLAGS
    <x.y.z> / --from     the stable version to verify (default: VERSION file)
    --apps a,b           restrict to a subset of the catalog (default: all)
    --artifact-timeout N prod artifactory artifact wait, seconds (default 600)
    --wait-timeout N     per-app pipeline wait, seconds (default 900)
    --poll-timeout N     per-app /version poll, seconds (default 600)
    --skip-artifact      skip the prod artifactory presence check

FLOW
    1. stable gate   — refuse anything that is not a plain x.y.z (rc is
                       covered by the rc chain; never here)
    2. prod artifactory — poll HEAD until the Upload PROD artifact is live
    3. per app       — trigger the app's main pipeline (pipeline-run
                       --wait --expect-success) then poll /version until
                       app_version == x.y.z. Order: integration-02
                       (clean-install check) first, then integration-01 /
                       devine-test-004 / devine-test-001.

EXIT
    0  every selected app verified on the stable
    1  gate failure, artifact timeout, or any app failed
"#;

/// The `apps` usage block.
pub const APPS: &str = r#"workflow apps — list the test FAAS apps catalog.

USAGE
    devine-utils-ci workflow apps [--output text|json]

EXAMPLES
    devine-utils-ci workflow apps
"#;

/// The `release_rc` usage block.
pub const RELEASE_RC: &str = r#"workflow release-rc — bump rc on the binary repo main + push + verify Upload UAT only.

USAGE
    devine-utils-ci workflow release-rc [--timeout SECONDS] [--no-auto-checkout]
                                        [--dry-run] [--verbose] [--output text|json]

FLOW
    1. checkout main + fetch/pull --ff-only (automatic unless --no-auto-checkout)
    2. bump rc on the binary repo (VERSION + mirror files + CHANGELOG.md when present)
    3. git push <remote> HEAD
    4. git push <remote> <tag>     # triggers Upload UAT; RC has no PROD job
    5. anonymous HEAD poll on UAT artifactory (≤ --timeout s, defaults 1800 =
       30 min). HEAD must be 200 + Content-Length > MinSizeB
       (default 1 MB; a built linux-amd64 devine-golang-mcp-app is ~10 MB).

NOTES
    A declared or auto-detected changelog must contain [Unreleased]; the rc
    marker is committed with VERSION so tag-time release gates stay atomic.
    Tag push triggers Upload UAT. RC tags explicitly do not create Upload PROD.
    The HEAD poll is token-free and confirms the UAT artifact is live.
"#;

/// The `test_rc` usage block.
pub const TEST_RC: &str = r#"workflow test-rc — trigger the dev-test pipeline and poll /version for app_version == rc.

USAGE
    devine-utils-ci workflow test-rc [--wait] [--wait-timeout SECONDS]
                                     [--poll-timeout SECONDS] [--output text|json]

FLOW
    1. POST /api/v4/projects/<dev-test-app>/pipeline (ref=main, no vars)
       — the install-server.sh in the faas-template auto-resolves the
       latest UAT rc binary (no version pinning needed; release-rc just
       published it to UAT).
    2. if --wait: poll pipeline status until terminal (--wait-timeout, default 600s)
    3. poll the dev-test app's /version endpoint (HTTP GET, every 5s,
       up to --poll-timeout, default 600s) until app_version matches the
       expected rc. This is the authoritative signal that the new binary
       is live — pipeline success alone is not enough (CI could succeed
       but the FAT env not yet rolled out).
"#;

/// The `integration_rc` usage block.
pub const INTEGRATION_RC: &str = r#"workflow integration-rc — trigger one or more integration FAAS apps.

USAGE
    devine-utils-ci workflow integration-rc [--app NAME] [--wait]
                                            [--wait-timeout SECONDS]
                                            [--poll-version]
                                            [--output text|json]

FLAGS
    --app NAME     one of: devine-test-004-function, devine-test-001-function,
                   devine-app-integration-01-function, devine-app-integration-02-function,
                   or role: dev-test, integration
                   (default: dev-test, i.e. devine-test-004-function)
    --poll-version also poll /version and assert app_version matches
                   the latest devine-golang-mcp-app release (uses --poll-timeout)

NOTES
    devine-app-integration-01-function has debug ON and may carry polluted
    QConfig keys from prior versions — useful for "do existing keys still
    work?" checks. devine-app-integration-02-function has debug OFF and
    no polluted config — the closest thing to a clean-install verification.
"#;

/// The `promote` usage block.
pub const PROMOTE: &str = r#"workflow promote — bump rc → stable on the binary repo main + push + verify.
This command enforces the release gates:

  Rule 1 (release gate):
    没有完成集成测试 01 的生产发布测试的，不允许发布正式版本.
    i.e. devine-app-integration-01-function MUST be serving the rc (via
    its /version endpoint) BEFORE this command will proceed. If you
    have not yet verified the rc on integration-01, run:

        devine-utils-ci workflow integration-rc --app devine-app-integration-01-function --poll-version

  Rule 2 (post-promote check):
    正式版本发布完成，必须用 integration-02 立刻测试.
    On success, this command automatically triggers
    devine-app-integration-02-function and polls its /version to confirm
    the new stable version is live. Disable only with --skip-post-check
    (not recommended).

USAGE
    devine-utils-ci workflow promote [--timeout SECONDS] [--no-auto-checkout]
                                     [--dry-run] [--from <rc-version>]
                                     [--poll-timeout SECONDS]
                                     [--skip-post-check] [--verbose]

FLAGS
    --from <x.y.z-rc.N>  the rc being promoted (default: current VERSION in the binary repo)
    --poll-timeout N     post-promote integration-02 /version poll timeout (default 600s)
    --skip-post-check    disable the mandatory integration-02 check (use with care)

FLOW (default)
    1. gate: GET devine-app-integration-01-function/version → must report app_version == rc
    2. checkout main + pull
    3. bump promote (x.y.z-rc.N → x.y.z)
    4. push main + tag vX.Y.Z
    5. poll PROD artifactory for vX.Y.Z
    6. post-check: trigger devine-app-integration-02-function pipeline + poll /version

FAILURE MODES
    gate fail:    error message tells you exactly which command to run.
    push fail:    nothing has been written (the rc is still rc).
    artifactory:  roll back via git reset + delete local tag.
    post-check:   the stable version IS live on PROD, but the rule requires
                  you to follow up with 'workflow integration-rc --app
                  devine-app-integration-02-function' if it didn't auto-run.
"#;

/// The `deploy_prod` usage block.
pub const DEPLOY_PROD: &str = r#"workflow deploy-prod — publish the latest main image to the app's Captain PROD groups.

TARGET RESOLUTION (catalog first, then FAAS):
    --app may name a catalog entry (dev-test/test1/integration-01/integration-02/role)
    OR any FAAS function repo path (e.g. faas/my-mcp-function). A non-catalog app is
    resolved from the FAAS registry (function → ksvcs) ...

HARD RESTRICTIONS (owner, 2026-10-10) — checked before any write:
    1. ONLY FAAS **devine golang** apps: pipeline_template.pipeline_code must
       start with "GolangDevine" (and language golang). nodejs / python /
       non-devine templates / plain repos are REFUSED.
    2. ONLY **non-core** apps: importance_level must be L4 or L5 (L1/L2/L3 are
       core applications). An absent/unknown level is refused (fail-closed).

Flow (mirrors the Captain UI steps, automated):
    1. resolve image: --image flag, else the latest SUCCESS image on the
       app's FAT release group (auto_deploy needs the exact image incl.
       timestamp suffix)
    2. for each ENABLED PROD group: POST /releases/auto_deploy → PENDING
    3. poll the group; auto POST /releases/<id>/rollout on PENDING
    4. require SUCCESS (REVERTED/REVOKED/FAILED = error)
    5. verify PROD /spec still exposes http_self_ping (spec-drift guard)

Clusters:
    test4        (dev-test)  -> 1 PROD group  SHAXY
    test1        (dev-test)  -> 1 PROD group  SHAXY  (public read-only MCP,
                                 no OAuth)
    integration-01            -> 2 PROD groups SHAXY + SHA-ALI (SGP DISABLED
                                 pending public-cloud image sync)
    integration-02            -> 2 PROD groups SHAXY + SGP (SHA-ALI disabled)

USAGE
    devine-utils-ci workflow deploy-prod [--app <name|role|FAAS repo path>] [--image <url>]
                                          [--captain-token <tok>] [--spec-tool T]
                                          [--timeout SECONDS] [--dry-run]
                                          [--verbose]

FLAGS
    --app NAME       dev-test (default) | test1 | integration-01 |
                   integration-02 | role   — OR a FAAS repo path
                   (faas/<name>-function): resolved from FAAS, devine-golang only
    --image URL      explicit image to deploy (default: latest FAT SUCCESS image)
    --captain-token  Captain UI x-user-token or OAuth access_token
                     (default: $CAPTAIN_USER_TOKEN). OAuth is sent as Bearer.
    --timeout N      per-group release wait timeout in seconds (default 900)
    --spec-tool T    PROD /spec tool to assert (default http_self_ping;
                     FAAS-resolved apps default to "ping")

NOTE
    Idempotent: re-running converges on SUCCESS for the same image.
"#;

/// The `scenarios` usage block.
pub const SCENARIOS: &str = r#"workflow scenarios — run the per-app integration scenario MCP tools across every environment.

USAGE
    devine-utils-ci workflow scenarios [--env fat|prod|all] [--app NAME] [--tool NAME]
                                       [--strict] [--dry-run] [--verbose] [--output text|json]

WHAT
    Every integration app publishes its integration coverage as MCP tools, one
    tool per scenario (defined in that app's src/inttest/):

        it_downstream_routing     downstream URL routing (FAT→FAT / PROD→PROD)
        it_mcp_chain              MCP cross-instance chain
        it_version_consistency    both sides on the same build
        it_health_checks          /healthz + smoke checks ping

    The tools address peers by SHORT NAME (no environment domain), so the
    runtime derives the domain from the deployment environment — the same YAML
    proves FAT→FAT and PROD→PROD. This command asks each app in each
    environment to run each scenario and aggregates the verdicts, so a single
    invocation covers 所有环境.

    FAT comes from the catalog's VersionURL, PROD from ProdURL.

EXIT
    0  every (app, env, scenario) PASS (MISSING alone still exits 0 without --strict)
    non-zero (1)  usage error, or any FAIL/ERROR, or MISSING while --strict is set

EXAMPLES
    devine-utils-ci workflow scenarios
    devine-utils-ci workflow scenarios --env prod
    devine-utils-ci workflow scenarios --tool it_downstream_routing
    devine-utils-ci workflow scenarios --strict --output json
"#;

/// The `status` usage block.
pub const STATUS: &str = r#"workflow status — one-shot snapshot: VERSION, latest tags, artifactory, test apps.

USAGE
    devine-utils-ci workflow status [--output text|json]

SHOWS
    - local binary repo: current VERSION + HEAD commit + branch
    - latest 3 local tags (vX.Y.Z or vX.Y.Z-rc.N)
    - prod artifactory: top 3 published versions
    - test apps: each /version endpoint (app_version, time, time)
"#;
