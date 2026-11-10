// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `release-model` command: the documented release DAG.
//!
//! The Go original keeps a frozen copy of the release-model document so that
//! `devine-utils-ci release-model` renders byte-for-byte identically to the
//! version shipped with `devine_utils` 0.11.7. Text mode prints that document
//! verbatim; `--output json|toon` emits the machine-readable model instead.

use crate::config::Config;
use crate::output::Report;

/// The human-readable DAG documentation, preserved verbatim.
const RELEASE_MODEL_TEXT: &str = r#"
# devine Release Models

## Simple (main→tag) — 唯一模型 (devine_groups, 2026-08-18 起)
  dev_xxx ──MR──▶ main ──(bump_version.sh 打 tag)──▶ vX.Y.Z | vX.Y.Z-rc.N
  - tag 只打在 main 上的 single-parent bump commit
  - 只保护 main 一个分支；release / release/v* 不保护
  - cmd: release-finish (main)
  - tag protect: v[0-9]*.[0-9]*.[0-9]-*(30) + v[0-9]*.[0-9]*.[0-9]*(40)

## Strict (main→MR→release→tag) — 已废弃 (2026-08-18)
  release / release/v* 分支已解除保护，不再作为强制发布线。

## 版本规范: rc pre-release (2026-08-18 起强制)
  - stable: x.y.z            (e.g. 1.1.2)
  - rc:     x.y.z-rc.N       (e.g. 1.1.3-rc.1, 1.1.3-rc.2)
  - 所有版本升级统一走: stable → rc.N → rc.N+1 → ... → promote → stable
      1. bump-version rc        # 1.1.2 → 1.1.3-rc.1 ; 1.1.3-rc.1 → 1.1.3-rc.2
      2. bump-version promote   # 1.1.3-rc.N → 1.1.3 (正式版, 无 rc 后缀)
  - tag: 稳定版 vX.Y.Z, rc 版 vX.Y.Z-rc.N —— 都只打在 main 的 single-parent bump commit

## Tag naming & rule priority (first match wins)
  pattern                        | create | example
  v[0-9]*.[0-9]*.[0-9]-*         | 30     | v0.2.0-rc.1, v0.2.0-alpha
  v[0-9]*.[0-9]*.[0-9]*          | 40     | v0.2.0, v10.20.0

## Cmd compat
  cmd                              | simple | strict
  release-start patch              | yes    | yes (deprecated)
  release-start --release-model    | -      | yes (deprecated)
  release-finish                   | yes    | -
  release-finish --release-model   | -      | yes (deprecated)
  release-check                    | yes    | yes
  bump-version patch|minor|major   | yes (stable only)
  bump-version rc                  | yes    | yes
  bump-version promote             | yes    | -
"#;

/// The JSON/TOON payload. Numbers stay numbers so an agent reasoning about
/// tag-protection access levels does not have to parse strings.
fn structured() -> serde_json::Value {
    serde_json::json!({
        "branch_protection": "main-only (2026-08-18): only main is protected; release / release/v* are unprotected",
        "cmd_compat": [
            {"cmd": "release-start patch", "simple": true, "strict": true},
            {"cmd": "release-start --release-model", "simple": false, "strict": true},
            {"cmd": "release-finish", "simple": true, "strict": false},
            {"cmd": "release-finish --release-model", "simple": false, "strict": true},
            {"cmd": "release-check", "simple": true, "strict": true},
            {"cmd": "bump-version patch|minor|major", "note": "rc must promote first", "stable-only": true},
            {"cmd": "bump-version rc", "note": "stable -> x.y.z+1-rc.1 ; rc.N -> rc.N+1"},
            {"cmd": "bump-version promote", "note": "x.y.z-rc.N -> x.y.z"},
        ],
        "models": [
            {
                "branch_protect": {"main": {"merge": 30, "push": 0}},
                "cmd": "release-finish",
                "dag": "dev_xxx --MR--> main --(bump_version.sh tag)--> vX.Y.Z | vX.Y.Z-rc.N",
                "name": "simple",
                "repos": ["all devine_*"],
                "tag_on": "main",
                "tag_protect": [
                    {"create_access_level": 30, "pattern": "v[0-9]*.[0-9]*.[0-9]-*"},
                    {"create_access_level": 40, "pattern": "v[0-9]*.[0-9]*.[0-9]*"},
                ],
            },
            {
                "branch_protect": {
                    "main": {"merge": 30, "push": 0},
                    "release": {"merge": 30, "push": 0},
                },
                "cmd": "release-finish --release-model strict",
                "dag": "dev_xxx --MR--> main --MR--> release --tag--> vX.Y.Z",
                "deprecated": "2026-08-18: release branches are no longer protected",
                "name": "strict",
                "repos": ["(deprecated)"],
                "tag_on": "release",
                "tag_protect": [
                    {"create_access_level": 30, "pattern": "v[0-9]*.[0-9]*.[0-9]-*"},
                    {"create_access_level": 40, "pattern": "v[0-9]*.[0-9]*.[0-9]*"},
                ],
            },
        ],
        "version_convention": "rc-pre-release (2026-08-18): stable x.y.z, rc x.y.z-rc.N; all upgrades go rc -> stable (promote)",
    })
}

/// Prints the human-readable form. Exposed for callers (and tests) that want
/// the document without a [`Config`].
pub fn release_model_doc() -> &'static str {
    RELEASE_MODEL_TEXT
}

/// The `release-model` command.
pub fn release_model(cfg: &Config) -> Result<i32, String> {
    if cfg.output.is_empty() {
        print!("{RELEASE_MODEL_TEXT}");
        return Ok(0);
    }
    let mut report = Report::new("release-model");
    report.ok = true;
    report.summary =
        "release-model: single-main + rc-pre-release convention (2026-08-18)".to_string();
    report.extra = Some(structured());
    print!("{}", report.render(cfg.output_mode()));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_document_is_preserved_verbatim() {
        let doc = release_model_doc();
        assert!(doc.starts_with("\n# devine Release Models\n"));
        assert!(doc.ends_with("  bump-version promote             | yes    | -\n"));
    }

    #[test]
    fn the_structured_payload_has_both_models() {
        let value = structured();
        let models = value.get("models").and_then(|m| m.as_array()).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0]["name"], "simple");
        assert_eq!(models[1]["name"], "strict");
        assert!(!value["branch_protection"].as_str().unwrap().is_empty());
    }
}
