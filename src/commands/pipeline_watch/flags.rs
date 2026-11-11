// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `pipeline-watch` flag set.
//!
//! The Go original hand-rolls this parser so the command can own a flat
//! flag list without dragging in the CI-gate-oriented global config. The
//! same rules apply here: an unknown flag is ignored, and a flag whose
//! value is missing falls back to the default.

/// The parsed flags of `pipeline-watch`.
#[derive(Debug, Clone, Default)]
pub struct WatchFlags {
    /// The config file to read; empty means the conventional path.
    pub config: String,
    /// The refresh interval in seconds; 0 keeps the configured value.
    pub interval: i64,
    /// How many pipelines each row shows; 0 keeps the configured value.
    pub per_repo: i64,
    /// Overrides the configured instance URL.
    pub base_url: String,
    /// Emits a one-shot JSON snapshot instead of the dashboard.
    pub json: bool,
}

/// Parses the flags the Go original accepts.
pub fn parse_flags(args: &[String]) -> WatchFlags {
    let mut flags = WatchFlags::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                if index + 1 < args.len() {
                    index += 1;
                    flags.config = args[index].clone();
                }
            }
            "--interval" => {
                if index + 1 < args.len() {
                    index += 1;
                    let value = atoi_or_zero(&args[index]);
                    if value > 0 {
                        flags.interval = value;
                    }
                }
            }
            "--per-repo" => {
                if index + 1 < args.len() {
                    index += 1;
                    let value = atoi_or_zero(&args[index]);
                    if value > 0 {
                        flags.per_repo = value;
                    }
                }
            }
            "--base-url" => {
                if index + 1 < args.len() {
                    index += 1;
                    flags.base_url = args[index].clone();
                }
            }
            "--json" => flags.json = true,
            _ => {}
        }
        index += 1;
    }
    flags
}

/// Parses a non-negative decimal number.
///
/// Anything that is not a plain digit sequence reads as zero, which is what
/// the Go original's `atoiOrZero` does — a negative or padded value never
/// overrides the configured one.
fn atoi_or_zero(text: &str) -> i64 {
    let mut value: i64 = 0;
    for ch in text.chars() {
        if !ch.is_ascii_digit() {
            return 0;
        }
        value = value
            .saturating_mul(10)
            .saturating_add(i64::from(ch as u8 - b'0'));
    }
    value
}

#[cfg(test)]
mod tests;
