// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the gather half of `version-check`.

use std::path::Path;

use super::*;

fn fixture(text: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join(VERSION_FILE), text).expect("VERSION");
    dir
}

#[test]
fn a_missing_version_file_is_worded_the_way_go_words_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let error = read_root_version(dir.path()).expect_err("no VERSION");
    assert_eq!(
        error,
        format!(
            "VERSION file not found: open {}: no such file or directory",
            dir.path().join(VERSION_FILE).display()
        )
    );
}

#[test]
fn the_bom_is_removed_before_the_whitespace_is_trimmed() {
    let dir = fixture("\u{feff}  1.2.3 \n");
    assert_eq!(read_root_version(dir.path()).expect("VERSION"), "1.2.3");
}

#[test]
fn only_the_two_legal_shapes_classify() {
    assert_eq!(classify("1.2.3"), Some(false));
    assert_eq!(classify("1.2.3-rc.7"), Some(true));
    assert_eq!(classify("1.2"), None);
    assert_eq!(classify("1.2.3-rc.x"), None);
    assert_eq!(classify("1.2.3.4"), None);
    assert_eq!(classify("v1.2.3"), None);
    // Go runs every component through `strconv.Atoi`, which rejects an
    // integer that does not fit rather than wrapping or truncating it.
    assert_eq!(classify("99999999999999999999.0.0"), None);
}

#[test]
fn the_version_go_mirror_is_optional() {
    let dir = fixture("1.2.3");
    let error = check_version_go(dir.path(), "1.2.3").expect_err("no version.go");
    assert!(
        error.starts_with(&format!(
            "pkg/version/version.go not found: open {}: ",
            dir.path().join("pkg/version/version.go").display()
        )),
        "{error}"
    );
}

#[test]
fn the_version_go_mirror_must_carry_the_root_version() {
    let dir = fixture("1.2.3");
    let path = dir.path().join("pkg/version/version.go");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, "package version\n\nvar VERSION = \"9.9.9\"\n").expect("write");
    let error = check_version_go(dir.path(), "1.2.3").expect_err("mismatch");
    assert_eq!(
        error,
        "pkg/version/version.go has VERSION=9.9.9 but VERSION file has 1.2.3"
    );
    std::fs::write(&path, "package version\n\nvar VERSION = \"1.2.3\"\n").expect("write");
    check_version_go(dir.path(), "1.2.3").expect("match");
}

#[test]
fn a_declared_version_mirror_must_exist_and_match() {
    let dir = fixture("1.2.3");
    let root = dir.path();
    let project = ProjectConfig {
        version_mirrors: vec!["pkg/version/VERSION".to_string()],
        ..ProjectConfig::default()
    };
    let mut info = Info::default();
    let error = check_version_mirrors(root, "1.2.3", &project, &mut info).expect_err("missing");
    assert_eq!(
        error,
        format!(
            "declared mirror \"pkg/version/VERSION\" does not exist at {} — fix .devine-utils-ci.yaml or create the file (drift is unsafe)",
            root.join("pkg/version/VERSION").display()
        )
    );
    assert!(!info.mirrors.is_empty());
    assert_eq!(info.mirrors[0].note, "missing");

    let path = root.join("pkg/version/VERSION");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, "0.9.9\n").expect("write");
    let mut info = Info::default();
    let error = check_version_mirrors(root, "1.2.3", &project, &mut info).expect_err("mismatch");
    assert_eq!(
        error,
        "mirror \"pkg/version/VERSION\" content (\"0.9.9\") does not match root VERSION (\"1.2.3\") — drift"
    );

    std::fs::write(&path, "1.2.3\n").expect("write");
    let mut info = Info::default();
    check_version_mirrors(root, "1.2.3", &project, &mut info).expect("match");
    assert!(info.mirrors[0].matched);
}

#[test]
fn outside_a_repository_the_branch_checks_stay_untouched() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut info = Info {
        version: "1.2.3".to_string(),
        ..Info::default()
    };
    gather_git(dir.path(), &mut info).expect("not a repository");
    assert!(!info.in_git);
    assert!(info.branch.is_empty());
    assert!(!info.is_rc);
}

#[test]
fn the_stable_base_is_the_version_without_the_rc_suffix() {
    assert_eq!(strip_rc("1.2.3-rc.4"), "1.2.3");
    assert_eq!(strip_rc("1.2.3"), "1.2.3");
}

#[test]
fn the_root_version_file_is_the_name_the_tool_owns() {
    let root = Path::new("/tmp");
    assert_eq!(root.join(VERSION_FILE), Path::new("/tmp/VERSION"));
}
