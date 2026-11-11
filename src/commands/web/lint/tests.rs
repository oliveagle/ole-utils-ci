// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `web-lint` command-line tests.

use super::*;
use crate::commands::web::lint::render;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|value| (*value).to_string()).collect()
}

/// A scratch directory that is removed when the test ends.
struct Scratch {
    root: std::path::PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("web-lint-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch");
        Self { root }
    }

    fn write(&self, name: &str, body: &str) -> std::path::PathBuf {
        let path = self.root.join(name);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        std::fs::write(&path, body).expect("write");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn the_usage_block_matches_go() {
    let usage = flag_set().render_usage("web-lint");
    let expected = [
        "Usage of web-lint:",
        "  -content string",
        "    \tinline source to lint (requires --language)",
        "  -external-timeout-sec int",
        "    \ttimeout for the external tsc run (default 60)",
        "  -fail-on string",
        "    \terror|warning|none — severity that makes exit code 1 (default \"error\")",
        "  -format string",
        "    \ttext|json|toon (default \"text\")",
        "  -ignore value",
        "    \tcomma-separated path substrings to skip (repeatable)",
        "  -language string",
        "    \thtml|css|js|ts|tsx|jsx (required with --content)",
        "  -max-file-size int",
        "    \tper-file byte cap (default 1048576)",
        "  -max-files int",
        "    \tcap on files linted per run (default 200)",
        "  -path string",
        "    \tfile or directory to lint (mutually exclusive with --content)",
        "  -project-root string",
        "    \tproject root; also the default security root (default \".\")",
        "  -root string",
        "    \tsecurity root: resolved paths must stay inside",
        "  -ts-engine string",
        "    \tauto|builtin|external (default \"auto\")",
        "  -tsconfig string",
        "    \ttsconfig.json path for the external tsc engine",
        "",
    ];
    assert_eq!(usage, expected.join("\n"));
}

#[test]
fn a_bad_flag_exits_two() {
    assert_eq!(web_lint(&args(&["--nope"])).expect("ran"), 2);
}

#[test]
fn help_exits_zero() {
    assert_eq!(web_lint(&args(&["-h"])).expect("ran"), 0);
}

#[test]
fn a_bad_format_exits_two() {
    assert_eq!(web_lint(&args(&["--format=bogus"])).expect("ran"), 2);
}

#[test]
fn inline_content_is_linted_under_its_language_name() {
    let mut fs = flag_set();
    fs.parse(&args(&["--content", "const a = 1;", "--language", "js"]))
        .expect("parse");
    let options = Options::from(&fs);
    let envelope = super::run::run(&options).expect("clean content");
    assert_eq!(envelope, 0);
}

#[test]
fn content_without_a_language_is_refused() {
    assert_eq!(
        web_lint(&args(&["--content", "const a = 1;"])).expect("ran"),
        2
    );
}

#[test]
fn a_missing_input_is_refused() {
    assert_eq!(web_lint(&args(&[])).expect("ran"), 2);
}

#[test]
fn a_broken_file_fails_the_run() {
    let scratch = Scratch::new("broken");
    scratch.write("bad.js", "function f() {\n");
    let root = scratch.root.to_string_lossy().to_string();
    assert!(web_lint(&args(&["--path", &root, "--root", &root])).is_err());
    assert_eq!(
        web_lint(&args(&["--path", &root, "--root", &root, "--fail-on=none"])).expect("ran"),
        0
    );
}

#[test]
fn ignore_substrings_skip_files() {
    let scratch = Scratch::new("ignore");
    scratch.write("bad.js", "function f() {\n");
    let root = scratch.root.to_string_lossy().to_string();
    assert_eq!(
        web_lint(&args(&[
            "--path",
            &root,
            "--root",
            &root,
            "--ignore=bad.js"
        ]))
        .expect("ran"),
        0
    );
}

#[test]
fn the_text_rendering_names_the_position_and_the_counters() {
    let finding = crate::commands::web::engine::Finding {
        line: 2,
        col: 1,
        severity: "error",
        rule: "js/syntax".to_string(),
        message: "Unexpected end of file".to_string(),
    };
    let results = vec![(
        "bad.js".to_string(),
        render::result("bad.js", "js", vec![finding]),
    )];
    let text = render::text(&render::envelope(&results));
    assert_eq!(
        text,
        "bad.js:2:1  error  js/syntax  Unexpected end of file\n\
         summary: 1 file(s), 1 error(s), 0 warning(s), 0 skipped\n"
    );
}

#[test]
fn fail_on_decides_the_verdict() {
    let results = vec![(
        "bad.js".to_string(),
        render::read_failure("bad.js", "js", &std::io::Error::other("boom")),
    )];
    let envelope = render::envelope(&results);
    assert_eq!(render::summary(&envelope, "skipped"), 1);
    assert!(!render::fails(&envelope, "error"));
    assert!(!render::fails(&envelope, "warning"));
}
