// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the global configuration.

use super::*;

fn argv(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn parse(args: &[&str]) -> Result<(Config, Vec<String>), String> {
    let mut cfg = Config::new();
    let mut fs = FlagSet::new();
    cfg.register(&mut fs);
    let rest = cfg.parse(&mut fs, &argv(args))?;
    Ok((cfg, rest))
}

#[test]
fn defaults_match_the_go_tool() {
    let (cfg, _) = parse(&[]).unwrap();
    assert_eq!(cfg.coverage_threshold, 80);
    assert_eq!(cfg.mode, Mode::Ci);
    assert_eq!(cfg.max_file_lines, 500);
    assert_eq!(cfg.max_complexity, 20);
    assert_eq!(cfg.complexity_warn, 15);
    assert_eq!(cfg.junit_path, "junit.xml");
}

#[test]
fn the_default_environment_is_the_full_one() {
    let (cfg, _) = parse(&[]).unwrap();
    assert_eq!(cfg.mode, Mode::Ci);
    assert!(!cfg.is_dev());
}

#[test]
fn the_environment_flag_selects_dev() {
    let (cfg, _) = parse(&["--mode", "dev"]).unwrap();
    assert_eq!(cfg.mode, Mode::Dev);
    assert!(cfg.is_dev());
}

#[test]
fn release_is_still_the_name_of_the_full_run() {
    assert_eq!(Mode::parse("release"), Mode::Ci);
    assert_eq!(Mode::parse("  DEV "), Mode::Dev);
    assert_eq!(Mode::parse("ci"), Mode::Ci);
    // A typo must not buy the weaker run.
    assert_eq!(Mode::parse("dve"), Mode::Ci);
    assert_eq!(Mode::parse(""), Mode::Ci);
}

#[test]
fn rejects_thresholds_above_the_built_in_ceilings() {
    assert!(parse(&["--max-file-lines=900"]).is_err());
    assert!(parse(&["--max-complexity=50"]).is_err());
    assert!(parse(&["--complexity-warn=30"]).is_err());
    assert!(parse(&["--coverage-threshold=10"]).is_err());
}

#[test]
fn accepts_tighter_thresholds() {
    let (cfg, _) = parse(&["--max-file-lines=300", "--complexity-warn=8"]).unwrap();
    assert_eq!(cfg.max_file_lines, 300);
    assert_eq!(cfg.complexity_warn, 8);
}

#[test]
fn rejects_unknown_output_modes() {
    assert!(parse(&["--output=yaml"]).is_err());
}

#[test]
fn project_root_is_resolved_to_an_absolute_path() {
    let (cfg, _) = parse(&[]).unwrap();
    assert!(cfg.project_root.is_absolute());
}

#[test]
fn the_o_alias_and_coverage_profile_reach_the_config() {
    let (cfg, _) = parse(&["-o", "json", "--coverage-profile", "cover.out"]).unwrap();
    assert_eq!(cfg.output, "json");
    assert_eq!(cfg.cover_profile, "cover.out");
}

#[test]
fn cover_profile_is_not_a_flag_go_knows() {
    // Go registers `coverage-profile` only; the old spelling must be refused.
    let err = parse(&["--cover-profile", "cover.out"]).unwrap_err();
    assert!(err.contains("flag provided but not defined: -cover-profile"));
}

#[test]
fn max_file_lines_set_follows_the_long_spelling() {
    let (cfg, _) = parse(&["--max-file-lines=500"]).unwrap();
    assert!(cfg.max_file_lines_set);
    let (cfg, _) = parse(&[]).unwrap();
    assert!(!cfg.max_file_lines_set);
}
