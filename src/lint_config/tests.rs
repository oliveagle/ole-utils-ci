// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the project configuration.

use super::*;
use std::path::Path;

#[test]
fn a_missing_file_is_the_default_policy() {
    let config = load(Path::new("/nonexistent-config-for-tests")).expect("load");
    let resolved = resolve(Path::new("/nonexistent-config-for-tests"), &config);
    assert_eq!(resolved.go_pin.version_file, ".go-version");
    assert_eq!(resolved.no_shell.dirs, vec!["cmd", "internal"]);
    assert_eq!(resolved.plugin.source_dir, "src/golang");
    assert_eq!(resolved.first_party_dirs, DEFAULT_FIRST_PARTY_DIRS.to_vec());
    assert!(resolved.go_format && resolved.go_mod_format && resolved.go_vet);
    assert!(resolved.golangci_lint);
    assert!(!resolved.no_shell.enabled);
    assert!(!resolved.web.enabled);
}

#[test]
fn the_lint_block_is_read() {
    let config: ProjectConfig = serde_yaml::from_str(
        "lint:\n  go_pin:\n    version_file: VERSION.go\n  no_shell_exec:\n    dirs: [a]\n",
    )
    .expect("parse");
    let resolved = resolve(Path::new("/tmp"), &config);
    assert_eq!(resolved.go_pin.version_file, "VERSION.go");
    assert_eq!(resolved.no_shell.dirs, vec!["a"]);
}

#[test]
fn an_explicit_switch_beats_the_default() {
    let config: ProjectConfig =
        serde_yaml::from_str("lint:\n  go_vet: false\n  first_party_dirs: [only]\n")
            .expect("parse");
    let resolved = resolve(Path::new("/tmp"), &config);
    assert!(!resolved.go_vet);
    assert_eq!(resolved.first_party_dirs, vec!["only"]);
}

#[test]
fn the_toolchain_pin_switches_itself_on_when_the_file_exists() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join(".go-version"), "1.25.0\n").expect("version file");
    let config = load(dir.path()).expect("load");
    let resolved = resolve(dir.path(), &config);
    assert!(resolved.go_pin.enabled);
}

#[test]
fn the_web_pass_defaults_to_the_whole_project() {
    let config: ProjectConfig =
        serde_yaml::from_str("lint:\n  web:\n    enabled: true\n").expect("parse");
    let resolved = resolve(Path::new("/tmp"), &config);
    assert!(resolved.web.enabled);
    assert_eq!(resolved.web.paths, vec!["."]);
    assert_eq!(resolved.web.ts_engine, "auto");
    assert_eq!(resolved.web.fail_on, "error");
}
