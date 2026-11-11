// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The per-surface checks: one file, one question each.
//!
//! Every function reads at most one file and returns the findings for it, in
//! the order the Go original appends them. A file that cannot be read for
//! anything but "does not exist" aborts the whole check, because the pin is
//! then unknown rather than merely drifted.

use std::path::Path;

use regex::Regex;

use super::{base_name, group, read_text, Pin, Settings};
use crate::output::Diagnostic;
use crate::static_regex;

pub(super) fn semver_re() -> Regex {
    static_regex::compile(r"^\d+\.\d+\.\d+$")
}

fn image_re() -> Regex {
    static_regex::compile(r"golang:(\d+\.\d+\.\d+)")
}

pub(super) fn go_image_re() -> Regex {
    static_regex::compile(r#"GO_IMAGE:\s*"[^"]*golang:(\d+\.\d+\.\d+)""#)
}

fn go_directive_re() -> Regex {
    static_regex::compile(r"(?m)^go\s+(\d+\.\d+)")
}

fn toolchain_re() -> Regex {
    static_regex::compile(r"(?m)^toolchain\s")
}

fn docker_from_re() -> Regex {
    static_regex::compile(r"(?m)^\s*FROM\s+\S*golang:(\d+\.\d+\.\d+)")
}

fn local_re() -> Regex {
    static_regex::compile(r#"(?m)^\s*GOTOOLCHAIN:\s*"?local"?\s*$"#)
}

fn hardcode_re() -> Regex {
    static_regex::compile(r#"REQUIRED_GO_VERSION="\d+\.\d+\.\d+""#)
}

/// Validates `.go-version`, the pin every other surface is compared against.
pub(super) fn read_version(root: &Path, version_file: &str) -> Result<Pin, String> {
    let raw = match read_text(&root.join(version_file))? {
        Some(raw) => raw,
        None => {
            return Ok(Pin::Blocked(vec![Diagnostic::error(
                version_file,
                "go-pin/version-missing",
                "Go version source file is missing",
            )]))
        }
    };
    let want = raw.trim().to_string();
    if !semver_re().is_match(&want) {
        return Ok(Pin::Blocked(vec![Diagnostic::error(
            version_file,
            "go-pin/version-format",
            format!("must be full semver x.y.z, got {want:?}"),
        )]));
    }
    let want_mm = match want.rfind('.') {
        Some(index) => want[..index].to_string(),
        None => want.clone(),
    };
    Ok(Pin::Ready(want, want_mm))
}

/// Checks the CI image, every hardcoded image reference and `GOTOOLCHAIN`.
pub(super) fn check_ci(
    root: &Path,
    settings: &Settings,
    want: &str,
) -> Result<Vec<Diagnostic>, String> {
    let mut findings = Vec::new();
    let content = match read_text(&root.join(&settings.ci_file))? {
        Some(content) => content,
        None => {
            findings.push(Diagnostic::error(
                &settings.ci_file,
                "go-pin/ci-missing",
                "CI file is missing",
            ));
            return Ok(findings);
        }
    };
    match go_image_re().captures(&content) {
        None => findings.push(Diagnostic::error(
            &settings.ci_file,
            "go-pin/ci-image-missing",
            "GO_IMAGE with a golang:x.y.z tag was not found",
        )),
        Some(captures) => {
            let found = group(&captures, 1);
            if found != want {
                findings.push(Diagnostic::error(
                    &settings.ci_file,
                    "go-pin/ci-image",
                    format!("GO_IMAGE uses golang:{found}, expected golang:{want}"),
                ));
            }
        }
    }
    let mut any_image = false;
    for (index, line) in content.lines().enumerate() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        for captures in image_re().captures_iter(line) {
            any_image = true;
            let found = group(&captures, 1);
            if found != want {
                findings.push(
                    Diagnostic::error(
                        &settings.ci_file,
                        "go-pin/ci-hardcoded-image",
                        format!("hardcoded golang:{found}, expected golang:{want}"),
                    )
                    .at_line(index + 1),
                );
            }
        }
    }
    if !any_image {
        findings.push(Diagnostic::error(
            &settings.ci_file,
            "go-pin/ci-image-missing",
            "no golang:x.y.z image reference was found",
        ));
    }
    if !local_re().is_match(&content) {
        findings.push(Diagnostic::error(
            &settings.ci_file,
            "go-pin/gotoolchain",
            "GOTOOLCHAIN: local is required so auto-upgrade cannot defeat the pin",
        ));
    }
    Ok(findings)
}

/// Checks every `FROM .../golang:x.y.z` stage of the Dockerfile.
pub(super) fn check_dockerfile(
    root: &Path,
    settings: &Settings,
    want: &str,
) -> Result<Vec<Diagnostic>, String> {
    let mut findings = Vec::new();
    if settings.dockerfile.is_empty() {
        return Ok(findings);
    }
    let content = match read_text(&root.join(&settings.dockerfile))? {
        Some(content) => content,
        None => return Ok(findings),
    };
    let mut stages = 0usize;
    for captures in docker_from_re().captures_iter(&content) {
        stages += 1;
        let found = group(&captures, 1);
        if found != want {
            findings.push(Diagnostic::error(
                &settings.dockerfile,
                "go-pin/docker-image",
                format!("FROM uses golang:{found}, expected golang:{want}"),
            ));
        }
    }
    if stages == 0 {
        findings.push(Diagnostic::error(
            &settings.dockerfile,
            "go-pin/docker-image-missing",
            "no FROM .../golang:x.y.z stage was found",
        ));
    }
    Ok(findings)
}

/// Checks that `require-go` reads the pin instead of hardcoding it.
pub(super) fn check_require_go(
    root: &Path,
    settings: &Settings,
) -> Result<Vec<Diagnostic>, String> {
    let mut findings = Vec::new();
    if settings.require_go_script.is_empty() {
        return Ok(findings);
    }
    let content = match read_text(&root.join(&settings.require_go_script))? {
        Some(content) => content,
        None => {
            findings.push(Diagnostic::error(
                &settings.require_go_script,
                "go-pin/require-go-missing",
                "require-go script is missing",
            ));
            return Ok(findings);
        }
    };
    if hardcode_re().is_match(&content) {
        findings.push(Diagnostic::error(
            &settings.require_go_script,
            "go-pin/require-go-hardcode",
            "REQUIRED_GO_VERSION is hardcoded; it must read the version file",
        ));
    }
    if !content.contains(&base_name(&settings.version_file)) {
        findings.push(Diagnostic::error(
            &settings.require_go_script,
            "go-pin/require-go-source",
            format!("must reference {}", settings.version_file),
        ));
    }
    Ok(findings)
}

/// Checks the `go` and `toolchain` directives of `go.mod`.
pub(super) fn check_gomod(
    root: &Path,
    want: &str,
    want_mm: &str,
) -> Result<Vec<Diagnostic>, String> {
    let mut findings = Vec::new();
    let content = match read_text(&root.join("go.mod"))? {
        Some(content) => content,
        None => {
            findings.push(Diagnostic::error(
                "go.mod",
                "go-pin/go-mod-missing",
                "go.mod is missing",
            ));
            return Ok(findings);
        }
    };
    match go_directive_re().captures(&content) {
        None => findings.push(Diagnostic::error(
            "go.mod",
            "go-pin/go-directive-missing",
            "go directive is missing",
        )),
        Some(captures) => {
            let found = group(&captures, 1);
            if found != want_mm {
                findings.push(Diagnostic::error(
                    "go.mod",
                    "go-pin/go-directive",
                    format!("go directive is {found}, expected {want_mm} ({want})"),
                ));
            }
        }
    }
    if toolchain_re().is_match(&content) {
        findings.push(Diagnostic::error(
            "go.mod",
            "go-pin/toolchain-directive",
            "toolchain directive can override the pin",
        ));
    }
    Ok(findings)
}
