// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `web-lint` scanner tests.

use super::*;

#[test]
fn detects_unbalanced_brackets() {
    assert!(check_brackets("js", "function a() {").is_some());
    assert!(check_brackets("js", "function a() {}").is_none());
}

#[test]
fn ignores_brackets_in_strings_and_comments() {
    assert!(check_brackets("js", "const a = '{';").is_none());
    assert!(check_brackets("js", "// {\nconst a = 1;").is_none());
}

#[test]
fn detects_unclosed_html_tags() {
    assert!(check_html_tags("<div><p>text</p></div>").is_none());
    assert!(check_html_tags("<div><p>text</div>").is_some());
    assert!(check_html_tags("<img src=\"x\"><br>").is_none());
}

#[test]
fn positions_a_finding_where_the_problem_is() {
    let finding = check_brackets("js", "function f() {\n").expect("finding");
    assert_eq!((finding.line, finding.col), (2, 1));
    assert_eq!(finding.rule, "js/syntax");
    assert_eq!(finding.message, "Unexpected end of file");
    let finding = check_brackets("js", "const a = 1;\n)").expect("finding");
    assert_eq!((finding.line, finding.col), (2, 1));
    assert_eq!(finding.message, "Unexpected \")\"");
}

#[test]
fn names_the_language_and_the_engine() {
    assert_eq!(language_of(Path::new("a/b/index.HTML")), "html");
    assert_eq!(language_of(Path::new("a/b/app.mjs")), "js");
    assert_eq!(language_of(Path::new("a/b/app.tsx")), "tsx");
    assert_eq!(language_of(Path::new("a/b/app.txt")), "");
    assert_eq!(engine_of("jsx"), "builtin-jsx");
    assert_eq!(inline_name("ts"), "inline.ts");
}
