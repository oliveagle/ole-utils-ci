// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The argument and environment helpers the hook commands share.
//!
//! The hooks are called by git rather than by a person, so the flags are
//! few and their values are simple. Every helper here is forgiving about
//! the spelling of a flag — `--max-files 3` and `--max-files=3` mean the
//! same thing — and every helper that reads the environment treats an
//! empty value as unset.

/// Reads an environment variable, treating an empty value as unset.
pub fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Parses `20`, `512k`, `5m` or `1g` into a byte count.
pub fn parse_size(raw: &str) -> Result<u64, String> {
    let raw = raw.trim();
    let (digits, factor) = match raw.chars().last() {
        Some('k' | 'K') => (&raw[..raw.len() - 1], 1024),
        Some('m' | 'M') => (&raw[..raw.len() - 1], 1024 * 1024),
        Some('g' | 'G') => (&raw[..raw.len() - 1], 1024 * 1024 * 1024),
        _ => (raw, 1),
    };
    let value: u64 = digits
        .trim()
        .parse()
        .map_err(|_| format!("{raw:?} is not a size — try 20, 512k, 5m or 1g"))?;
    Ok(value * factor)
}

/// Lifts `--name <value>` and `--name=<value>` out of the argument list.
pub fn take_value(args: &[String], name: &str) -> (Option<String>, Vec<String>) {
    let long = format!("--{name}");
    let mut found = None;
    let mut rest = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == &long && index + 1 < args.len() {
            found = Some(args[index + 1].clone());
            index += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix(&format!("{long}=")) {
            found = Some(value.to_string());
            index += 1;
            continue;
        }
        rest.push(arg.clone());
        index += 1;
    }
    (found, rest)
}

/// Lifts a boolean `--name` (or `--name=false`) out of the argument list.
pub fn take_switch(args: &[String], name: &str) -> (bool, Vec<String>) {
    let long = format!("--{name}");
    let mut found = false;
    let mut rest = Vec::new();
    for arg in args {
        if arg == &long {
            found = true;
        } else if let Some(value) = arg.strip_prefix(&format!("{long}=")) {
            found = value != "false";
        } else {
            rest.push(arg.clone());
        }
    }
    (found, rest)
}

/// Formats a byte count the way the hook messages show it.
pub fn human_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    let bytes_f = bytes as f64;
    if bytes_f >= KIB * KIB {
        format!("{:.1} MiB", bytes_f / (KIB * KIB))
    } else if bytes_f >= KIB {
        format!("{:.1} KiB", bytes_f / KIB)
    } else {
        format!("{bytes} B")
    }
}
