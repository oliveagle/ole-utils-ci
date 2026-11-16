// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Small helpers shared by the MoonBit engine and its callers.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.contains("failed to execute `git"))
        .unwrap_or("command failed without output")
        .to_string()
}

pub(super) fn inside_build(rel: &str) -> bool {
    rel.split('/')
        .any(|part| matches!(part, "_build" | ".repos" | ".mooncakes" | "target" | "dist"))
}

pub(super) fn temp_path(extension: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "devine-utils-ci-moonbit-{}-{nanos}.{extension}",
        std::process::id()
    ))
}
