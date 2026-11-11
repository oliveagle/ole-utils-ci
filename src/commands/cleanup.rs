// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `cleanup`: stop the background processes `devine-utils-ci` starts.
//!
//! CI jobs start helpers (`fake-metadata-server`) that outlive the step that
//! started them. This command is what an `after_script` or a `trap` calls to
//! release the ports again. It looks for the helpers three ways, because any
//! one of them can miss: the PID files the tool writes, the well-known port
//! the helper binds, and the process name.

use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::time::Duration;

/// Where the background helpers record their pids.
const PID_DIR: &str = "/tmp/devine-utils-ci-pids";

/// The ports the helpers bind, with the helper that owns each.
const KNOWN_PORTS: [(u16, &str); 1] = [(18080, "fake-metadata-server")];

/// The helper process names, for the last-resort sweep.
const HELPER_NAMES: [&str; 1] = ["fake-metadata-server"];

/// How long the port probe waits before deciding nothing is listening.
const PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// The `cleanup` command.
///
/// The Go original ignores its arguments, so this does too.
pub fn cleanup(_args: &[String]) -> Result<i32, String> {
    kill_from_pid_files();
    for (port, name) in KNOWN_PORTS {
        if kill_on_port(port).is_ok() {
            println!("  \u{00b7} killed {name} (port {port})");
        }
    }
    for name in HELPER_NAMES {
        if kill_by_name(name).is_ok() {
            println!("  \u{00b7} killed leftover {name} process");
        }
    }
    println!("\u{2705} Cleanup complete");
    Ok(0)
}

/// Kills every process a PID file names, then drops the directory.
fn kill_from_pid_files() {
    let Ok(entries) = std::fs::read_dir(PID_DIR) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".pid") else {
            continue;
        };
        let Ok(pid) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(pid) = pid.trim().parse::<i64>() else {
            continue;
        };
        if signal_kill(pid).is_ok() {
            println!("  \u{00b7} killed {stem} (pid {pid})");
        }
    }
    let _ = std::fs::remove_dir_all(PID_DIR);
}

/// Kills whatever listens on `port`.
///
/// The probe first: dialling the port is what tells the differences between
/// "nothing is running" and "something is running but `lsof` is missing".
fn kill_on_port(port: u16) -> Result<(), String> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    if TcpStream::connect_timeout(&address, PROBE_TIMEOUT).is_err() {
        return Err(format!("no process on port {port}"));
    }
    let output = Command::new("lsof")
        .arg("-ti")
        .arg(format!(":{port}"))
        .output()
        .map_err(|error| format!("find process on port {port}: {error}"))?;
    if !output.status.success() {
        return Err(format!("find process on port {port}: lsof failed"));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for token in text.split_whitespace() {
        if let Ok(pid) = token.trim().parse::<i64>() {
            let _ = signal_kill(pid);
        }
    }
    Ok(())
}

/// Kills every process whose command line contains `name`.
fn kill_by_name(name: &str) -> Result<(), String> {
    let status = Command::new("pkill")
        .arg("-f")
        .arg(name)
        .status()
        .map_err(|error| format!("no {name} process found: {error}"))?;
    if status.success() {
        return Ok(());
    }
    Err(format!("no {name} process found"))
}

/// Sends SIGKILL.
///
/// `kill(1)` rather than `libc::kill` keeps the crate free of `unsafe`.
fn signal_kill(pid: i64) -> Result<(), String> {
    let status = Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| format!("kill {pid}: {error}"))?;
    match status.success() {
        true => Ok(()),
        false => Err(format!("kill {pid}: no such process")),
    }
}

#[cfg(test)]
mod tests;
