// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The work `web-lint` does once its command line has been accepted.

use std::path::{Path, PathBuf};

use super::render;
use super::Options;
use crate::fsutil;
use crate::output;

use super::super::engine::{self, Finding};

/// Why `web-lint` stopped before it could report.
#[derive(Debug)]
pub enum Failure {
    /// A refused request: exit 2 with both the coded and the bare message.
    Usage(UsageError),
    /// An unexpected failure: `main` prints `Error: {0}` and exits 1.
    Io(String),
}

/// A refusal the engine reports as `web-lint: <message> (<code>)`.
#[derive(Debug)]
pub struct UsageError {
    /// The message, without the code.
    pub message: String,
    /// The stable code Go appends in parentheses.
    pub code: &'static str,
}

impl Failure {
    /// A usage error dressed as a failure.
    fn usage(code: &'static str, message: impl Into<String>) -> Self {
        Failure::Usage(UsageError {
            message: message.into(),
            code,
        })
    }
}

impl From<std::io::Error> for Failure {
    fn from(error: std::io::Error) -> Self {
        Failure::Io(error.to_string())
    }
}

/// One linted file: its relative path and its JSON result object.
pub type Outcome = (String, serde_json::Value);

/// Validates the request, lints what it names and writes the result.
pub fn run(options: &Options) -> Result<i32, Failure> {
    let root = resolve_root(options);
    let mode = output::parse_mode(&options.format);
    let envelope = if options.content.is_empty() {
        let target = resolve_path(options, &root)?;
        lint_tree(options, &root, &target)?
    } else {
        lint_inline(options)?
    };
    render::write(mode, &envelope)?;
    if render::fails(&envelope, &options.fail_on) {
        return Err(Failure::Io(format!(
            "findings at or above --fail-on={}",
            options.fail_on
        )));
    }
    Ok(0)
}

/// The root the run resolves paths against.
fn resolve_root(options: &Options) -> PathBuf {
    let named = if options.root.is_empty() {
        &options.project_root
    } else {
        &options.root
    };
    fsutil::absolute(Path::new(named)).unwrap_or_else(|_| PathBuf::from(named))
}

/// The path to lint, resolved and checked to stay inside the root.
fn resolve_path(options: &Options, root: &Path) -> Result<PathBuf, Failure> {
    if options.path.is_empty() {
        return Err(Failure::usage(
            "WEB_LINT_NO_INPUT",
            "web_lint: provide path or content",
        ));
    }
    let named = Path::new(&options.path);
    let target = if named.is_absolute() {
        named.to_path_buf()
    } else {
        root.join(named)
    };
    let resolved = fsutil::absolute(&target).unwrap_or_else(|_| target.clone());
    if !contained(root, &resolved) {
        return Err(Failure::usage(
            "WEB_LINT_PATH_ESCAPES_ROOT",
            format!(
                "path {:?} escapes root {:?} (pass root= to widen)",
                options.path,
                root.display().to_string()
            ),
        ));
    }
    Ok(resolved)
}

/// Whether `path` stays inside `root`.
fn contained(root: &Path, path: &Path) -> bool {
    path == root || path.starts_with(root)
}

/// Lints one path — a file or a whole directory.
fn lint_tree(options: &Options, root: &Path, target: &Path) -> Result<serde_json::Value, Failure> {
    let metadata = std::fs::metadata(target)?;
    let mut results = Vec::new();
    if metadata.is_file() {
        let language = engine::language_of(target);
        if language.is_empty() {
            return Err(Failure::usage(
                "WEB_LINT_UNSUPPORTED_FILE",
                format!("unsupported file type: {}", target.display()),
            ));
        }
        let rel = fsutil::relative_slashed(root, target);
        results.push(lint_file(&rel, target, language));
    } else {
        let mut visited = 0i64;
        for entry in fsutil::walk_files(target) {
            let language = engine::language_of(&entry.path);
            if language.is_empty() || ignored(&entry.rel, &options.ignore) {
                continue;
            }
            if visited >= options.max_files {
                break;
            }
            let size = std::fs::metadata(&entry.path).map(|m| m.len()).unwrap_or(0) as i64;
            if size > options.max_file_size {
                results.push(skipped_result(&entry.rel, language));
                continue;
            }
            visited += 1;
            results.push(lint_file(&entry.rel, &entry.path, language));
        }
        results.sort_by(|a, b| a.0.cmp(&b.0));
    }
    Ok(render::envelope(&results))
}

/// Lints an inline snippet.
fn lint_inline(options: &Options) -> Result<serde_json::Value, Failure> {
    let language = match options.language.as_str() {
        known @ ("html" | "css" | "js" | "ts" | "tsx" | "jsx") => known,
        _ => {
            return Err(Failure::usage(
                "WEB_LINT_INVALID_LANGUAGE",
                "web_lint: language must be one of html|css|js|ts|tsx|jsx for content mode",
            ));
        }
    };
    let name = engine::inline_name(language);
    let findings = engine::lint(language, &options.content);
    Ok(render::envelope(&[(
        name.clone(),
        render::result(&name, language, findings),
    )]))
}

/// Lints one file on disk.
fn lint_file(rel: &str, path: &Path, language: &'static str) -> Outcome {
    match std::fs::read(path) {
        Ok(data) => {
            let source = String::from_utf8_lossy(&data);
            let findings: Vec<Finding> = engine::lint(language, &source);
            (rel.to_string(), render::result(rel, language, findings))
        }
        Err(error) => (rel.to_string(), render::read_failure(rel, language, &error)),
    }
}

/// The result object for a file above the byte cap.
fn skipped_result(rel: &str, language: &str) -> Outcome {
    (
        rel.to_string(),
        serde_json::json!({
            "file": rel,
            "language": language,
            "engine": "-",
            "issues": serde_json::Value::Array(Vec::new()),
            "skipped": "file exceeds max_file_size",
        }),
    )
}

/// Whether a path matches one of the `--ignore` substrings.
fn ignored(rel: &str, patterns: &[String]) -> bool {
    let lower = rel.to_ascii_lowercase();
    patterns
        .iter()
        .any(|pattern| !pattern.is_empty() && lower.contains(&pattern.to_ascii_lowercase()))
}
