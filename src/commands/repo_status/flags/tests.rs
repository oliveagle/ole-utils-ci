// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn argv(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn a_positional_keeps_its_place_but_the_flags_move_first() {
    let reordered = reorder_flags_first(&argv(&["devine/foo", "--pretty"]));
    assert_eq!(reordered, argv(&["--pretty", "devine/foo"]));
}

#[test]
fn a_value_flag_takes_the_next_token_with_it() {
    let reordered = reorder_flags_first(&argv(&["devine/foo", "-pipelines", "3", "--jobs"]));
    assert_eq!(
        reordered,
        argv(&["-pipelines", "3", "--jobs", "devine/foo"])
    );
}

#[test]
fn a_boolean_flag_leaves_the_next_token_alone() {
    // `--mrs devine/foo` must not read the path as the flag's value: that is
    // exactly the trap `isBoolFlag` exists for.
    let reordered = reorder_flags_first(&argv(&["--mrs", "devine/foo"]));
    assert_eq!(reordered, argv(&["--mrs", "devine/foo"]));
}

#[test]
fn an_inline_value_is_not_split_from_its_flag() {
    let reordered = reorder_flags_first(&argv(&["devine/foo", "--pipelines=3"]));
    assert_eq!(reordered, argv(&["--pipelines=3", "devine/foo"]));
}

#[test]
fn a_flag_before_a_flag_keeps_both() {
    let reordered = reorder_flags_first(&argv(&["--pretty", "--jobs", "devine/foo"]));
    assert_eq!(reordered, argv(&["--pretty", "--jobs", "devine/foo"]));
}

#[test]
fn the_project_path_survives_and_parses() {
    let (flags, positional) =
        parse(&argv(&["devine/utils", "--pipelines", "2", "--jobs"])).expect("parses");
    assert_eq!(positional, vec!["devine/utils".to_string()]);
    assert_eq!(flags.pipelines, 2);
    assert!(flags.include_jobs);
}

#[test]
fn the_pipeline_window_is_clamped_to_one_and_fifty() {
    let (low, _) = parse(&argv(&["--pipelines", "0"])).expect("parses");
    let (high, _) = parse(&argv(&["--pipelines", "999"])).expect("parses");
    assert_eq!(low.pipelines, 1);
    assert_eq!(high.pipelines, 50);
}

#[test]
fn a_non_positive_timeout_falls_back_to_thirty() {
    let (zero, _) = parse(&argv(&["-timeout", "0"])).expect("parses");
    let (negative, _) = parse(&argv(&["-timeout", "-5"])).expect("parses");
    assert_eq!(zero.timeout, 30);
    assert_eq!(negative.timeout, 30);
}

#[test]
fn trace_implies_jobs() {
    let (flags, _) = parse(&argv(&["--trace"])).expect("parses");
    assert!(flags.include_trace);
    assert!(flags.include_jobs);
}

#[test]
fn dash_o_is_a_shorthand_for_output() {
    let (flags, _) = parse(&argv(&["-o", "json"])).expect("parses");
    assert_eq!(flags.output, "json");
}

#[test]
fn the_last_output_flag_wins() {
    let (flags, _) = parse(&argv(&["--output", "json", "-o", "text"])).expect("parses");
    assert_eq!(flags.output, "text");
}

#[test]
fn an_unregistered_flag_is_refused() {
    let error = parse(&argv(&["--bogus"])).expect_err("refused");
    assert_eq!(error.to_string(), "flag provided but not defined: -bogus");
}

#[test]
fn a_bad_integer_is_refused_with_go_wording() {
    let error = parse(&argv(&["--pipelines", "abc"])).expect_err("refused");
    assert_eq!(
        error.to_string(),
        "invalid value \"abc\" for flag -pipelines: parse error"
    );
}

#[test]
fn help_is_a_help_request_not_a_value() {
    let error = parse(&argv(&["-h"])).expect_err("help");
    assert!(matches!(error, ParseError::Help));
}
