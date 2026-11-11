// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The template's `go test` line, in both forms it is needed: the argv the
//! pipe runner spawns, and the shell one-liner `--verbose` echoes.

use super::{Artifacts, Options};

/// The argv form of the template's `go test` line.
pub(super) fn test_args(options: &Options, paths: &Artifacts, build_tags: &str) -> Vec<String> {
    let mut args = vec![
        "go".to_string(),
        "test".to_string(),
        format!("-coverpkg={}", options.cover_pkg),
        "-v".to_string(),
        "-cover".to_string(),
        format!("-coverprofile={}", paths.profile.display()),
    ];
    if !build_tags.is_empty() {
        args.push(format!("-tags={build_tags}"));
    }
    args.push(options.test_packages.clone());
    if !options.no_vendor {
        args.push("-mod=vendor".to_string());
    }
    args
}

/// The same command rendered as the shell one-liner Captain-CI runs.
pub(super) fn test_command(options: &Options, paths: &Artifacts, build_tags: &str) -> String {
    let mut parts = vec![
        "go test".to_string(),
        format!("-coverpkg={}", options.cover_pkg),
        "-v".to_string(),
        "-cover".to_string(),
        format!(
            "-coverprofile={}",
            shell_quote(&paths.profile.to_string_lossy())
        ),
    ];
    if !build_tags.is_empty() {
        parts.push(format!("-tags={}", shell_quote(build_tags)));
    }
    parts.push(options.test_packages.clone());
    if !options.no_vendor {
        parts.push("-mod=vendor".to_string());
    }
    format!(
        "{} 2>&1 | go-junit-report -set-exit-code > {} || true",
        parts.join(" "),
        shell_quote(&paths.junit.to_string_lossy())
    )
}

/// Wraps a word in single quotes when it holds anything the shell would
/// otherwise interpret.
fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if !value.contains([' ', '\t', '\'', '"', '\\', '$', '`']) {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn opts() -> Options {
        Options {
            cover_pkg: "./...".to_string(),
            test_packages: "./...".to_string(),
            ..Default::default()
        }
    }

    fn paths(dir: &str) -> Artifacts {
        Artifacts::new(Path::new(dir))
    }

    #[test]
    fn the_argv_form_matches_the_template() {
        let args = test_args(&opts(), &paths("/o"), "");
        assert_eq!(
            args,
            vec![
                "go",
                "test",
                "-coverpkg=./...",
                "-v",
                "-cover",
                "-coverprofile=/o/cover.profile",
                "./...",
                "-mod=vendor",
            ]
        );
    }

    #[test]
    fn build_tags_and_no_vendor_change_both_forms() {
        let mut options = opts();
        options.no_vendor = true;
        let args = test_args(&options, &paths("/o"), "unit");
        assert!(args.contains(&"-tags=unit".to_string()));
        assert!(!args.contains(&"-mod=vendor".to_string()));

        let line = test_command(&options, &paths("/o"), "unit");
        assert_eq!(
            line,
            "go test -coverpkg=./... -v -cover -coverprofile=/o/cover.profile -tags=unit ./... 2>&1 | go-junit-report -set-exit-code > /o/junit.xml || true"
        );
    }

    #[test]
    fn a_path_with_a_space_is_quoted() {
        let line = test_command(&opts(), &paths("/a b"), "");
        assert!(
            line.contains("-coverprofile='/a b/cover.profile'"),
            "{line}"
        );
        assert!(line.contains("> '/a b/junit.xml'"), "{line}");
    }

    #[test]
    fn an_embedded_single_quote_is_closed_and_reopened() {
        assert_eq!(shell_quote("it's"), r#"'it'\''s'"#);
        assert_eq!(shell_quote("plain"), "plain");
        assert_eq!(shell_quote(""), "''");
    }
}
