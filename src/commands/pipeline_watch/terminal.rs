// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The terminal mode the interactive dashboard needs.
//!
//! The Go original uses `golang.org/x/term`, which reaches into libc
//! directly. This crate forbids `unsafe` in production code, so the same
//! job is done with `stty`: it is part of coreutils, it talks to the
//! controlling terminal, and it needs no memory tricks.

use std::io::IsTerminal;
use std::process::{Command, Stdio};

/// Whether stdin is a terminal.
pub fn stdin_is_tty() -> bool {
    std::io::stdin().is_terminal()
}

/// Puts the terminal into raw mode for as long as the guard lives.
///
/// Returns `None` when the terminal cannot be switched — a pipe, a CI
/// runner, a platform without `stty`. The caller then falls back to the
/// line-based reader.
pub struct RawMode {
    /// The settings to restore on drop.
    saved: Option<String>,
}

impl RawMode {
    /// Enters raw mode.
    pub fn enable() -> Option<RawMode> {
        let saved = stty(&["-g"]).ok()?;
        stty(&["raw", "-echo"]).ok()?;
        Some(RawMode { saved: Some(saved) })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        if let Some(saved) = self.saved.take() {
            let _ = stty(&[saved.as_str()]);
        }
    }
}

/// Runs `stty` against the controlling terminal.
fn stty(args: &[&str]) -> Result<String, String> {
    let output = Command::new("stty")
        .args(args)
        .stdin(Stdio::inherit())
        .output()
        .map_err(|error| format!("stty: {error}"))?;
    if !output.status.success() {
        return Err(format!("stty {} failed", args.join(" ")));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests;
