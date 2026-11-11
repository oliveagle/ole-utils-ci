// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `go-pin` gate: Go toolchain drift.
//!
//! `.go-version` is the single source of truth; every version-bearing
//! surface — the CI image, the Dockerfile, the `go` directive in `go.mod`,
//! the `require-go` script — must agree with it.
//!
//! The standalone command is one of the *focused* lint gates: it owns a
//! private flag set, defaults to TOON output, exits 2 when the command line
//! is refused and 1 when the pin has drifted. The per-surface checks live in
//! [`surfaces`], because each one reads one file and asks one question.

use std::path::Path;

use regex::Captures;

use crate::gates::focused;
use crate::lint_config;
use crate::output::Diagnostic;

mod surfaces;

/// The default name of the version file.
pub const DEFAULT_VERSION_FILE: &str = ".go-version";
/// The default name of the CI file.
pub const DEFAULT_CI_FILE: &str = ".gitlab-ci.yml";
/// The default name of the Dockerfile.
pub const DEFAULT_DOCKERFILE: &str = "Dockerfile";
/// The default name of the `require-go` script.
pub const DEFAULT_REQUIRE_GO_SCRIPT: &str = "scripts/ci/require-go.sh";

/// The surfaces one run of the gate checks.
#[derive(Debug, Clone)]
pub struct Settings {
    /// The single source of truth for the Go version.
    pub version_file: String,
    /// The GitLab pipeline file.
    pub ci_file: String,
    /// The container build file; empty skips the check.
    pub dockerfile: String,
    /// The `require-go` script; empty skips the check.
    pub require_go_script: String,
    /// Whether the running `go version` must agree too.
    pub with_toolchain: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            version_file: DEFAULT_VERSION_FILE.to_string(),
            ci_file: DEFAULT_CI_FILE.to_string(),
            dockerfile: DEFAULT_DOCKERFILE.to_string(),
            require_go_script: DEFAULT_REQUIRE_GO_SCRIPT.to_string(),
            with_toolchain: false,
        }
    }
}

/// Reads the version file, or the finding that stops the run.
enum Pin {
    /// The version as written, plus its `x.y` prefix.
    Ready(String, String),
    /// The pin itself is unusable; every later surface is skipped.
    Blocked(Vec<Diagnostic>),
}

/// Checks the toolchain pin across every surface.
///
/// `extra` is `None` when the pin itself is missing or malformed, which is
/// exactly when the Go original returns a nil payload.
pub fn check(
    root: &Path,
    settings: &Settings,
) -> Result<(Vec<Diagnostic>, Option<serde_json::Value>), String> {
    let (want, want_mm) = match surfaces::read_version(root, &settings.version_file)? {
        Pin::Ready(want, want_mm) => (want, want_mm),
        Pin::Blocked(findings) => return Ok((findings, None)),
    };
    let mut findings = Vec::new();
    findings.extend(surfaces::check_ci(root, settings, &want)?);
    findings.extend(surfaces::check_dockerfile(root, settings, &want)?);
    findings.extend(surfaces::check_require_go(root, settings)?);
    findings.extend(surfaces::check_gomod(root, &want, &want_mm)?);
    let running = check_toolchain(root, settings, &want, &mut findings);
    let with_toolchain = settings.with_toolchain;
    Ok((
        findings,
        Some(extra(settings, &want, &running, with_toolchain)),
    ))
}

/// The `extra` payload, with its keys in the sorted order Go's map encodes.
fn extra(
    settings: &Settings,
    want: &str,
    running: &str,
    with_toolchain: bool,
) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("platform".to_string(), serde_json::json!(platform()));
    if with_toolchain {
        map.insert("running_toolchain".to_string(), serde_json::json!(running));
    }
    map.insert("version".to_string(), serde_json::json!(want));
    map.insert(
        "version_file".to_string(),
        serde_json::json!(settings.version_file),
    );
    serde_json::Value::Object(map)
}

/// `runtime.GOOS/runtime.GOARCH`, in Go's spelling.
fn platform() -> String {
    format!("{}/{}", std::env::consts::OS, go_arch())
}

/// Maps Rust's architecture names onto Go's `GOARCH` values.
fn go_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "x86" => "386",
        "aarch64" => "arm64",
        other => other,
    }
}

/// `filepath.Base(path)`, with Go's `.` for an empty path.
fn base_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string())
}

/// Reads a text file; `None` means the file does not exist.
fn read_text(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read(path) {
        Ok(data) => Ok(Some(String::from_utf8_lossy(&data).to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

/// The first capture group of `captures`, or the empty string.
fn group(captures: &Captures<'_>, index: usize) -> String {
    captures
        .get(index)
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

/// Compares the running toolchain, when the caller asked for it.
///
/// Returns the version the running `go` reported, or the empty string when it
/// could not be determined — the value the report carries either way.
fn check_toolchain(
    root: &Path,
    settings: &Settings,
    want: &str,
    findings: &mut Vec<Diagnostic>,
) -> String {
    if !settings.with_toolchain {
        return String::new();
    }
    match running_go_version(root) {
        Ok(running) => {
            if running != want {
                findings.push(Diagnostic::error(
                    "",
                    "go-pin/running-toolchain",
                    format!("running go{running}, expected go{want}"),
                ));
            }
            running
        }
        Err(message) => {
            findings.push(Diagnostic::error("", "go-pin/running-toolchain", message));
            String::new()
        }
    }
}

/// The version a `go version` run in `dir` reports, without the `go` prefix.
fn running_go_version(dir: &Path) -> Result<String, String> {
    let output = std::process::Command::new("go")
        .arg("version")
        .current_dir(dir)
        .output()
        .map_err(|e| format!("run go version: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let fields: Vec<&str> = text.split_whitespace().collect();
    if fields.len() < 3 {
        return Err(format!("unexpected go version output: {}", text.trim()));
    }
    Ok(fields[2].trim_start_matches("go").to_string())
}

/// Runs the gate the way the Go command does: a private flag set, TOON
/// output by default, exit 2 for a refused command line and exit 1 for drift.
pub fn focused_command(args: &[String]) -> Result<i32, String> {
    let root = focused::root_default(args);
    let config = lint_config::load(&root)?;
    let resolved = lint_config::resolve(&config).go_pin;
    let mut fs = focused::base(&root, args);
    fs.str_flag("version-file", &resolved.version_file);
    fs.str_flag("ci-file", &resolved.ci_file);
    fs.str_flag("dockerfile", &resolved.dockerfile);
    fs.str_flag("require-go-script", &resolved.require_go_script);
    fs.bool_flag("with-toolchain", resolved.with_toolchain);
    fs.describe("version-file", "Go version source file");
    fs.describe("ci-file", "GitLab CI file");
    fs.describe("dockerfile", "Dockerfile (empty to skip)");
    fs.describe("require-go-script", "require-go script");
    fs.describe("with-toolchain", "also compare the running go version");
    let flags = match focused::finish("go-pin", args, fs) {
        Ok(flags) => flags,
        Err(failure) => return failure.into_result(),
    };
    let settings = Settings {
        version_file: flags.string("version-file"),
        ci_file: flags.string("ci-file"),
        dockerfile: flags.string("dockerfile"),
        require_go_script: flags.string("require-go-script"),
        with_toolchain: flags.bool("with-toolchain"),
    };
    let (findings, extra) = check(flags.root(), &settings)?;
    focused::render("go-pin", &findings, extra, flags.mode())?;
    Ok(0)
}

#[cfg(test)]
mod tests;
