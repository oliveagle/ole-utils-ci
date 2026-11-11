// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the tree-mirror comparison.

use std::path::Path;

use crate::lint_config::TreeMirror;

use super::*;

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(path, content).expect("write");
}

fn pair() -> (tempfile::TempDir, tempfile::TempDir) {
    (
        tempfile::tempdir().expect("src"),
        tempfile::tempdir().expect("dst"),
    )
}

fn kinds(diffs: &[Diff]) -> Vec<String> {
    diffs
        .iter()
        .map(|diff| format!("[{}] {} — {}", diff.kind, diff.rel, diff.detail))
        .collect()
}

#[test]
fn identical_trees_have_no_diff() {
    let (src, dst) = pair();
    write(src.path(), "a/b.txt", "hello");
    write(dst.path(), "a/b.txt", "hello");
    assert!(compare(src.path(), dst.path()).expect("compare").is_empty());
}

#[test]
fn a_file_only_on_the_left_is_missing_in_dst() {
    let (src, dst) = pair();
    write(src.path(), "a/b.txt", "hello");
    let diffs = compare(src.path(), dst.path()).expect("compare");
    // The parent directory is an entry of its own, exactly as Go walks it.
    assert_eq!(
        kinds(&diffs),
        vec![
            "[missing-in-dst] a — directory",
            "[missing-in-dst] a/b.txt — file, 5 bytes"
        ]
    );
}

#[test]
fn a_file_only_on_the_right_is_missing_in_src() {
    let (src, dst) = pair();
    write(dst.path(), "extra.txt", "");
    let diffs = compare(src.path(), dst.path()).expect("compare");
    assert_eq!(
        kinds(&diffs),
        vec!["[missing-in-src] extra.txt — file, 0 bytes"]
    );
}

#[test]
fn a_directory_only_on_the_left_is_described_as_a_directory() {
    let (src, dst) = pair();
    std::fs::create_dir_all(src.path().join("empty")).expect("mkdir");
    let diffs = compare(src.path(), dst.path()).expect("compare");
    assert_eq!(kinds(&diffs), vec!["[missing-in-dst] empty — directory"]);
}

#[test]
fn different_bytes_are_reported_as_the_two_sizes() {
    let (src, dst) = pair();
    // Same length, different bytes: Go still prints the two sizes.
    write(src.path(), "same.txt", "aaaa");
    write(dst.path(), "same.txt", "bbbb");
    let diffs = compare(src.path(), dst.path()).expect("compare");
    assert_eq!(kinds(&diffs), vec!["[content-diff] same.txt — size 4 != 4"]);

    write(dst.path(), "same.txt", "b");
    let diffs = compare(src.path(), dst.path()).expect("compare");
    assert_eq!(kinds(&diffs), vec!["[content-diff] same.txt — size 4 != 1"]);
}

#[test]
fn a_file_where_a_directory_sits_is_a_type_mismatch() {
    let (src, dst) = pair();
    write(src.path(), "thing", "file");
    std::fs::create_dir_all(dst.path().join("thing")).expect("mkdir");
    let diffs = compare(src.path(), dst.path()).expect("compare");
    assert_eq!(
        kinds(&diffs),
        vec!["[content-diff] thing — type mismatch (src file, dst dir)"]
    );
}

#[test]
fn the_summary_stops_after_five_entries() {
    let (src, dst) = pair();
    for index in 0..6 {
        write(&src.path().join("d"), &format!("f{index}.txt"), "x");
    }
    let diffs = compare(src.path(), dst.path()).expect("compare");
    let summary = summarise(&diffs, 5);
    // Six files plus the directory that holds them: seven entries, five shown.
    assert!(summary.ends_with("... (+2 more)"), "{summary}");
    assert_eq!(summary.matches("; ").count(), 5);
    assert_eq!(summarise(&[], 5), "(no diff)");
}

#[test]
fn a_missing_src_or_dst_is_a_configuration_error() {
    let root = tempfile::tempdir().expect("root");
    let mut project = ProjectConfig {
        tree_mirrors: vec![TreeMirror {
            src: "nowhere".to_string(),
            dsts: vec!["copy".to_string()],
        }],
        ..ProjectConfig::default()
    };
    let mut info = Info::default();
    let error = check(root.path(), &project, &mut info).expect_err("missing src");
    assert!(
        error.starts_with("tree mirror src \"nowhere\" does not exist at "),
        "{error}"
    );

    std::fs::create_dir_all(root.path().join("skill")).expect("mkdir");
    project.tree_mirrors[0].src = "skill".to_string();
    let error = check(root.path(), &project, &mut info).expect_err("missing dst");
    assert!(
        error.starts_with("tree mirror dst \"copy\" does not exist at ")
            && error.contains("run the project's sync command"),
        "{error}"
    );

    project.tree_mirrors[0].dsts = Vec::new();
    let error = check(root.path(), &project, &mut info).expect_err("no dsts");
    assert_eq!(
        error,
        "tree_mirrors src \"skill\" has no dsts — fix .devine-utils-ci.yaml"
    );

    project.tree_mirrors[0].dsts = vec!["   ".to_string()];
    let error = check(root.path(), &project, &mut info).expect_err("empty dst");
    assert_eq!(
        error,
        "tree_mirrors src \"skill\" has an empty dst — fix .devine-utils-ci.yaml"
    );
}

#[test]
fn drift_is_worded_the_way_the_error_message_words_it() {
    let root = tempfile::tempdir().expect("root");
    write(root.path(), "skill/a.txt", "one");
    write(root.path(), "copy/a.txt", "two");
    let project = ProjectConfig {
        tree_mirrors: vec![TreeMirror {
            src: "skill".to_string(),
            dsts: vec!["copy".to_string()],
        }],
        ..ProjectConfig::default()
    };
    let mut info = Info::default();
    let error = check(root.path(), &project, &mut info).expect_err("drift");
    assert_eq!(
        error,
        "tree mirror src \"skill\" is out of sync with dst \"copy\" — drift: [content-diff] a.txt — size 3 != 3"
    );
    assert!(!info.tree_mirrors[0].matched);

    std::fs::write(root.path().join("copy/a.txt"), "one").expect("write");
    let mut info = Info::default();
    check(root.path(), &project, &mut info).expect("in sync");
    assert!(info.tree_mirrors[0].matched);
}
