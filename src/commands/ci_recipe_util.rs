// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The download half of `ci-recipe`: fetch, gunzip, chmod, verify.
//!
//! Rust port of the Go original's `internal/ci_recipe_util.go` plus
//! `DownloadCIRecipeBinary` from `internal/ci_recipe.go`. The sequence is
//! the runtime twin of [`super::ci_recipe::CIRecipe::setup_script`]: a
//! failed HTTP exchange creates nothing, a non-gzip body never reaches
//! the destination file, and the installed binary is checked against the
//! tag before the command reports success.

use std::fs::File;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::ci_recipe::{go_dir, join_install, CIRecipe};
use crate::httpd;

/// The syscall wording Go prints for the errno values this command can
/// realistically hit; anything else falls back to Rust's own strerror
/// text, lower-cased the way `syscall.Errno.Error` spells it.
pub(crate) fn go_errno(error: &std::io::Error) -> String {
    match error.raw_os_error() {
        Some(1) => "operation not permitted".to_string(),
        Some(2) => "no such file or directory".to_string(),
        Some(13) => "permission denied".to_string(),
        Some(17) => "file exists".to_string(),
        Some(20) => "not a directory".to_string(),
        Some(21) => "is a directory".to_string(),
        Some(28) => "no space left on device".to_string(),
        Some(30) => "read-only file system".to_string(),
        Some(36) => "file name too long".to_string(),
        Some(40) => "too many levels of symbolic links".to_string(),
        _ => {
            let text = error.to_string();
            let text = match text.rfind(" (os error ") {
                Some(index) => &text[..index],
                None => text.as_str(),
            };
            let mut characters = text.chars();
            match characters.next() {
                Some(first) => {
                    let head: String = first.to_lowercase().collect();
                    format!("{head}{}", characters.as_str())
                }
                None => String::new(),
            }
        }
    }
}

/// `os.MkdirAll` with Go's `PathError` wording: the report names the
/// component that actually failed, not always the directory asked for
/// (`mkdir /etc/hostname/x: mkdir /etc/hostname: not a directory`).
pub(crate) fn mkdir_all(path: &str) -> Result<(), String> {
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.is_dir() {
            return Ok(());
        }
        return Err(format!("mkdir {path}: not a directory"));
    }
    let parent = go_dir(path);
    if parent != path {
        mkdir_all(&parent)?;
    }
    match std::fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(create_error) => {
            // Absorb a concurrent creator the way Go's retry does.
            if let Ok(metadata) = std::fs::metadata(path) {
                if metadata.is_dir() {
                    return Ok(());
                }
            }
            Err(format!("mkdir {path}: {}", go_errno(&create_error)))
        }
    }
}

/// `gzip.NewReader`'s refusal, before any file is created: an empty body
/// is `EOF`, a short one `unexpected EOF`, a wrong magic or method
/// `gzip: invalid header`. The reserved flag bits are *not* checked —
/// `compress/gzip` accepts them, and so does this probe.
fn probe_gzip_header(source: &[u8]) -> Result<(), String> {
    if source.len() < 10 {
        if source.is_empty() {
            return Err("EOF".to_string());
        }
        return Err("unexpected EOF".to_string());
    }
    if source[0] != 0x1f || source[1] != 0x8b || source[2] != 8 {
        return Err("gzip: invalid header".to_string());
    }
    Ok(())
}

/// Translates `gunzip`'s stderr into the wording `compress/gzip` would
/// produce. The byte offset Go reports for a corrupt deflate stream has
/// no equivalent in the external tool.
fn describe_gunzip_failure(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let text = text.trim();
    if text.contains("unexpected end of file") {
        return "unexpected EOF".to_string();
    }
    if text.contains("invalid compressed data--crc error")
        || text.contains("invalid compressed data--length error")
    {
        return "gzip: invalid checksum".to_string();
    }
    if text.contains("invalid compressed data--format violated") {
        return "flate: corrupt input".to_string();
    }
    if text.is_empty() {
        return "gunzip failed".to_string();
    }
    text.to_string()
}

/// `gunzipStreamToFile`: decompress `source` into `destination`.
///
/// The header is validated first, so a non-gzip body leaves no file
/// behind, exactly like Go's `gzip.NewReader` running before `os.Create`.
/// The stream itself goes through `gunzip -c`, the same binary the setup
/// script requires.
pub(crate) fn gunzip_stream_to_file(source: &[u8], destination: &str) -> Result<(), String> {
    probe_gzip_header(source).map_err(|error| format!("not a valid gzip stream: {error}"))?;
    let file = File::create(destination).map_err(|error| {
        format!(
            "create {destination}: open {destination}: {}",
            go_errno(&error)
        )
    })?;
    let mut child = Command::new("gunzip")
        .arg("-c")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(file))
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("copy: failed to run gunzip: {error}"))?;
    // A corrupt body makes gunzip exit before the whole stream is read;
    // its status and stderr carry the real reason, so a broken pipe here
    // is not the error to report.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(source);
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("copy: {error}"))?;
    if !output.status.success() {
        return Err(format!("copy: {}", describe_gunzip_failure(&output.stderr)));
    }
    Ok(())
}

/// `verifyInstalledBinary`: run `<path> version` and check the last
/// whitespace-separated token of its combined output. Shared with
/// `upgrade`, which re-checks the binary it installed.
pub(crate) fn verify_installed_binary(path: &str, expected_version: &str) -> Result<(), String> {
    let output = match Command::new(path).arg("version").output() {
        Ok(output) => output,
        Err(error) => {
            return Err(format!(
                "verify: {path} version failed: fork/exec {path}: {} (output: )",
                go_errno(&error)
            ));
        }
    };
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    let line = combined.trim();
    if !output.status.success() {
        let reason = match output.status.code() {
            Some(code) => format!("exit status {code}"),
            None => "signal: killed".to_string(),
        };
        return Err(format!(
            "verify: {path} version failed: {reason} (output: {line})"
        ));
    }
    if line.is_empty() {
        return Err(format!("verify: {path} version produced no output"));
    }
    let got = line.split_whitespace().last().unwrap_or("");
    if got != expected_version {
        return Err(format!(
            "version mismatch: want={expected_version} got={got} (full: {line})"
        ));
    }
    Ok(())
}

/// `DownloadCIRecipeBinary`: fetch, gunzip, chmod and optionally verify —
/// the runtime twin of the setup script. Returns the installed path.
///
/// The HTTP failure path creates nothing: `MkdirAll` runs only after a
/// 2xx status, the way `curl -f` aborts before touching the destination.
pub fn download_ci_recipe_binary(recipe: &CIRecipe) -> Result<String, String> {
    recipe.validate()?;
    if recipe.job_token.is_empty() {
        return Err("ci-recipe: JobToken is empty (set $CI_JOB_TOKEN)".to_string());
    }
    let url = recipe.artifact_url()?;
    let response = httpd::request(
        "GET",
        &url,
        &[("JOB-TOKEN", recipe.job_token.as_str())],
        None,
        Duration::from_secs(60),
    )
    .map_err(|error| format!("ci-recipe: HTTP: {error}"))?;
    if !(200..300).contains(&response.status) {
        let peek = &response.body[..response.body.len().min(512)];
        return Err(format!(
            "ci-recipe: HTTP {} ({}): {}",
            response.status,
            response.status_display(),
            String::from_utf8_lossy(peek).trim()
        ));
    }
    let install_dir = if recipe.install_dir.is_empty() {
        "bin"
    } else {
        recipe.install_dir.as_str()
    };
    mkdir_all(install_dir).map_err(|error| format!("ci-recipe: mkdir {install_dir}: {error}"))?;
    let installed = join_install(install_dir, "devine-utils-ci");
    gunzip_stream_to_file(&response.body, &installed)
        .map_err(|error| format!("ci-recipe: gunzip: {error}"))?;
    std::fs::set_permissions(&installed, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("ci-recipe: chmod: chmod {installed}: {}", go_errno(&error)))?;
    if recipe.expect_verify {
        let expected = recipe.version.strip_prefix('v').unwrap_or(&recipe.version);
        verify_installed_binary(&installed, expected)?;
    }
    Ok(installed)
}

#[cfg(test)]
mod tests;
