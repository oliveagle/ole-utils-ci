// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}

#[test]
fn each_value_flag_accepts_both_spellings() {
    let (options, rest) = split_args(&args(&[
        "--coverpkg",
        "./svc/...",
        "--test-packages=./pkg/...",
        "--output-dir",
        "_artifacts",
        "extra",
    ]));
    assert_eq!(options.cover_pkg, "./svc/...");
    assert_eq!(options.test_packages, "./pkg/...");
    assert_eq!(options.output_dir, "_artifacts");
    assert_eq!(rest, args(&["extra"]));
}

#[test]
fn the_two_switches_are_exact_matches() {
    let (options, rest) = split_args(&args(&["--no-vendor", "--skip-tool-install"]));
    assert!(options.no_vendor);
    assert!(options.skip_tool_install);
    assert!(rest.is_empty());

    // `--no-vendor=true` is not the Go loop's spelling, so it goes through.
    let (options, rest) = split_args(&args(&["--no-vendor=true"]));
    assert!(!options.no_vendor);
    assert_eq!(rest, args(&["--no-vendor=true"]));
}

#[test]
fn a_value_flag_without_a_value_reaches_the_global_parser() {
    let (options, rest) = split_args(&args(&["--coverpkg"]));
    assert!(options.cover_pkg.is_empty());
    assert_eq!(rest, args(&["--coverpkg"]));
}

#[test]
fn an_empty_inline_value_is_kept_and_defaulted_later() {
    let (options, rest) = split_args(&args(&["--coverpkg="]));
    assert!(options.cover_pkg.is_empty());
    assert!(rest.is_empty());
}

#[test]
fn the_global_flags_pass_through_untouched() {
    let (options, rest) = split_args(&args(&[
        "--project-root",
        "svc",
        "--verbose",
        "--build-tags=unit",
    ]));
    assert_eq!(options.cover_pkg, "");
    assert_eq!(
        rest,
        args(&["--project-root", "svc", "--verbose", "--build-tags=unit"])
    );
}

#[test]
fn the_defaults_are_the_go_ones() {
    let mut cfg = Config::new();
    cfg.project_root = PathBuf::from("/repo");
    let options = with_defaults(&cfg, Options::default());
    assert_eq!(options.cover_pkg, "./...");
    assert_eq!(options.test_packages, "./...");
    assert_eq!(options.output_dir, "/repo");
    assert!(!options.no_vendor);
    assert!(!options.skip_tool_install);
}

#[test]
fn an_explicit_output_dir_wins_over_the_project_root() {
    let mut cfg = Config::new();
    cfg.project_root = PathBuf::from("/repo");
    let options = with_defaults(
        &cfg,
        Options {
            output_dir: "_artifacts".to_string(),
            ..Default::default()
        },
    );
    assert_eq!(
        output_dir(&cfg, &options.output_dir),
        PathBuf::from("/repo/_artifacts")
    );
    let absolute = PathBuf::from("/tmp/out");
    assert_eq!(output_dir(&cfg, "/tmp/out"), absolute);
}

#[test]
fn a_path_under_the_root_is_shown_relative() {
    let root = std::env::temp_dir().join(format!("duci-rel-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("temp dir");
    let nested = root.join("artifacts").join("junit.xml");
    assert_eq!(rel_to(&root, &nested), "artifacts/junit.xml");
    assert_eq!(
        rel_to(&root, &PathBuf::from("/elsewhere/x")),
        "/elsewhere/x"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_missing_artifact_reports_the_go_path_error() {
    let missing = PathBuf::from("/definitely/not/here");
    let error = std::fs::metadata(&missing).expect_err("must be missing");
    assert_eq!(
        path_error("stat", &missing, &error),
        "stat /definitely/not/here: no such file or directory"
    );
}

#[test]
fn the_banner_is_the_go_one() {
    assert_eq!(
        BANNER,
        "🚀 Running Captain-CI Test job (mirrors docs/pipeline/ci_template_v0.yaml)"
    );
}
