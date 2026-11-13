// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `hook commit-msg`: the commit message must say what the change is.
//!
//! Git hands the hook the path of `.git/COMMIT_EDITMSG`, which still carries
//! the comment lines the editor showed. They are stripped before the message
//! is judged, so a commit written in an editor and one written with `-m` are
//! checked the same way.

use std::path::Path;

use crate::output::Diagnostic;

use super::policy::Policy;
use super::{parse_globals, Verdict};

/// Runs `hook commit-msg <path>`.
pub fn commit_msg(args: &[String]) -> Result<i32, String> {
    let (policy, rest) = Policy::parse(args)?;
    let (cfg, positional) = parse_globals(&rest)?;
    let Some(path) = positional.first() else {
        return Err(
            "commit-msg needs the message file — git passes .git/COMMIT_EDITMSG".to_string(),
        );
    };
    let raw = std::fs::read_to_string(Path::new(path))
        .map_err(|e| format!("cannot read the commit message {path}: {e}"))?;
    let message = clean(&raw);
    let header = format!(
        "📝 commit-msg: {}",
        first_line(&message).unwrap_or("<empty>")
    );
    let mut verdict = Verdict::new("hook commit-msg");
    match policy.message_problem(&message) {
        None => verdict.pass("the message follows the commit convention"),
        Some(problem) => {
            verdict.deny(Diagnostic::error(path, "commit-message", &problem), problem);
        }
    }
    let extra = serde_json::json!({
        "message_file": path,
        "types": super::policy::COMMIT_TYPES,
        "max_subject_chars": super::policy::SUBJECT_MAX,
    });
    Ok(verdict
        .with_extra(extra)
        .finish(&cfg, &header, "commit-msg passed"))
}

/// Drops the comment lines git adds and the blank lines around them.
fn clean(raw: &str) -> String {
    let kept: Vec<&str> = raw
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();
    kept.join("\n").trim().to_string()
}

/// The first line a reader would call the subject.
fn first_line(message: &str) -> Option<&str> {
    message.lines().map(str::trim).find(|line| !line.is_empty())
}
