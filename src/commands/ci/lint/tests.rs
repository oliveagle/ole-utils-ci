// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `ci-lint` static checks.

use super::*;

fn rules(text: &str) -> Vec<String> {
    let parsed = serde_yaml::from_str::<Value>(text).expect("parse");
    let mut found = scan_text(text);
    found.extend(scan_jobs(&parsed));
    found.extend(scan_folded(text));
    found.into_iter().map(|issue| issue.rule).collect()
}

#[test]
fn reports_tabs_and_trailing_whitespace() {
    // A tab makes the document unparseable, which is a YAML error of its own,
    // so the text scan is what reports it.
    assert!(scan_text("job:\n\tscript: echo hi\n")
        .iter()
        .any(|issue| issue.rule == "tab-char"));
    assert!(scan_text("job:  \n")
        .iter()
        .any(|issue| issue.rule == "trailing-whitespace"));
}

#[test]
fn reports_an_unquoted_colon() {
    let found = rules("job:\n  - echo a: b\n");
    assert!(found.contains(&"unquoted-colon".to_string()));
    let found = rules("job:\n  - \"echo a: b\"\n");
    assert!(!found.contains(&"unquoted-colon".to_string()));
}

#[test]
fn a_job_without_a_script_is_rejected() {
    assert!(rules("job:\n  stage: test\n").contains(&"no-script".to_string()));
    assert!(rules("job: null\n").contains(&"empty-job".to_string()));
}

#[test]
fn templates_are_exempt_from_the_shape_check() {
    let found = rules(".template:\n  stage: test\n");
    assert!(!found.contains(&"no-script".to_string()));
    assert!(!found.contains(&"empty-job".to_string()));
}

#[test]
fn a_script_must_be_strings() {
    assert!(rules("job:\n  script:\n    - echo hi\n    - 42\n")
        .contains(&"script-not-string".to_string()));
    assert!(!rules("job:\n  script: echo hi\n").contains(&"script-not-string".to_string()));
}

#[test]
fn a_folded_block_scalar_in_a_script_is_rejected() {
    let text = "job:\n  script:\n    >\n    echo one\n    echo two\n";
    assert!(rules(text).contains(&"folded-block-scalar".to_string()));
}
