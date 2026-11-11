// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the `upgrade` command.
//!
//! Every networked test stands up a loopback server, so the suite runs
//! without touching the real upstream instance.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;

/// `&["a", "b"]` -> `["a".to_string(), "b".to_string()]`.
fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

/// `gzip -c` of `raw`, the fixture compressor this machine ships with.
fn gzip_bytes(raw: &[u8]) -> Vec<u8> {
    let mut child = Command::new("gzip")
        .arg("-c")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("gzip is installed");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(raw)
        .expect("gzip input written");
    let output = child.wait_with_output().expect("gzip finishes");
    assert!(output.status.success(), "gzip -c failed");
    output.stdout
}

/// A shell stub that answers `version` the way the real binary does.
fn stub_binary(version: &str) -> Vec<u8> {
    format!("#!/bin/sh\necho devine-utils-ci version {version}\n").into_bytes()
}

/// Another parallel test's `fork` can inherit our fixture write descriptor
/// for the microseconds before its `exec` closes it, which makes our own
/// `exec` of that file see `ETXTBSY`. Retry while the window is open.
fn retry_busy<T>(mut operation: impl FnMut() -> Result<T, String>) -> Result<T, String> {
    let mut attempts = 0;
    loop {
        match operation() {
            Err(error) if error.to_lowercase().contains("text file busy") && attempts < 50 => {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(2));
            }
            result => return result,
        }
    }
}

/// A loopback server that routes each request through `handler`, which
/// receives the request path and this server's own origin, until
/// [`Server::shutdown`] stops it.
struct Server {
    /// The `http://host:port` prefix of the served URL.
    base_url: String,
    /// Tells the accept loop to exit.
    stop: Arc<AtomicBool>,
    /// The accept loop itself.
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    /// Stops the accept loop and waits for it to finish.
    fn shutdown(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn serve<F>(handler: F) -> Server
where
    F: Fn(&str, &str) -> (String, Vec<u8>) + Send + Sync + 'static,
{
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback bind");
    listener
        .set_nonblocking(true)
        .expect("non-blocking listener");
    let address = listener.local_addr().expect("local address");
    let origin = format!("http://{address}");
    let thread_origin = origin.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let handle = std::thread::spawn(move || loop {
        let accepted = match listener.accept() {
            Ok((stream, _)) => Some(stream),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(_) => break,
        };
        if let Some(mut stream) = accepted {
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => request.extend_from_slice(&buffer[..read]),
                }
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let head = String::from_utf8_lossy(&request).to_string();
            let path = head
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .split('?')
                .next()
                .unwrap_or("/")
                .to_string();
            let (status, body) = handler(&path, &thread_origin);
            let header = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
            continue;
        }
        if thread_stop.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    });
    Server {
        base_url: origin,
        stop,
        handle: Some(handle),
    }
}

/// A config pointed at `base_url`, already tagged and platform-checked.
fn config_for(base_url: &str, version: &str) -> UpgradeConfig {
    UpgradeConfig {
        version: version.to_string(),
        dir: "bin".to_string(),
        arch: go_arch().to_string(),
        os: go_os().to_string(),
        verify: false,
        base_url: base_url.to_string(),
        project_id: DEFAULT_PROJECT_ID.to_string(),
        token: "test-token".to_string(),
    }
}

#[test]
fn a_leading_positional_is_the_version_and_flags_follow_it() {
    let cfg = parse_upgrade_flags(&args(&["v0.3.13", "--arch", "arm64", "--dir", "/opt"]))
        .expect("parses");
    assert_eq!(cfg.version, "v0.3.13");
    assert_eq!(cfg.arch, "arm64");
    assert_eq!(cfg.dir, "/opt");
    assert_eq!(cfg.os, "");
    assert!(cfg.verify, "Go defaults --verify to true");
    assert_eq!(cfg.base_url, DEFAULT_BASE_URL);
    assert_eq!(cfg.project_id, DEFAULT_PROJECT_ID);
}

#[test]
fn flags_before_a_positional_leave_the_version_empty() {
    // Go's flag package stops at the first non-flag token, and the
    // leading positional is only peeled off before parsing begins.
    let cfg = parse_upgrade_flags(&args(&["--arch", "arm64", "v0.3.13"])).expect("parses");
    assert_eq!(cfg.version, "");
    assert_eq!(cfg.arch, "arm64");
}

#[test]
fn parse_refusals_carry_go_wording() {
    let error = parse_upgrade_flags(&args(&["--bogus"])).expect_err("unknown flag");
    assert_eq!(error, "flag provided but not defined: -bogus");
    let error = parse_upgrade_flags(&args(&["--arch"])).expect_err("missing value");
    assert_eq!(error, "flag needs an argument: -arch");
    let error = parse_upgrade_flags(&args(&["-h"])).expect_err("help");
    assert_eq!(error, "flag: help requested");
    let error = parse_upgrade_flags(&args(&["--verify=maybe"])).expect_err("bad bool");
    assert_eq!(
        error,
        "invalid boolean value \"maybe\" for -verify: parse error"
    );
}

#[test]
fn validate_fills_in_the_host_platform() {
    if !is_valid_os(go_os()) || !is_valid_arch(go_arch()) {
        return;
    }
    let mut cfg = parse_upgrade_flags(&args(&["v9.9.9"])).expect("parses");
    cfg.validate().expect("valid tag");
    assert_eq!(cfg.os, go_os());
    assert_eq!(cfg.arch, go_arch());
}

#[test]
fn validate_rejects_a_tag_that_is_not_semver() {
    let mut cfg = parse_upgrade_flags(&args(&["bad"])).expect("parses");
    let error = cfg.validate().expect_err("bad tag");
    assert_eq!(error, "version \"bad\" must match vX.Y.Z");
    let mut cfg = parse_upgrade_flags(&args(&["v1.2.3-rc.1"])).expect("parses");
    let error = cfg.validate().expect_err("rc tag");
    assert_eq!(error, "version \"v1.2.3-rc.1\" must match vX.Y.Z");
}

#[test]
fn validate_rejects_platforms_the_release_does_not_carry() {
    let mut cfg = config_for("http://127.0.0.1:1", "v1.2.3");
    cfg.arch = "ppc".to_string();
    let error = cfg.validate().expect_err("bad arch");
    assert_eq!(error, "unsupported arch \"ppc\" (supported: amd64, arm64)");

    let mut cfg = config_for("http://127.0.0.1:1", "v1.2.3");
    cfg.os = "plan9".to_string();
    let error = cfg.validate().expect_err("bad os");
    assert_eq!(
        error,
        "unsupported OS \"plan9\" (supported: linux, darwin, windows)"
    );
}

#[test]
fn upgrades_installs_and_verifies_from_a_local_release() {
    if !is_valid_os(go_os()) || !is_valid_arch(go_arch()) {
        return;
    }
    let name = format!("devine-utils-ci-{}-{}.gz", go_os(), go_arch());
    let artifact = gzip_bytes(&stub_binary("9.9.9"));
    let server = serve(move |path, origin| {
        if path.starts_with("/api/v4/projects/158442/releases/") {
            let body = format!(
                r#"{{"assets":{{"links":[{{"name":"{name}","url":"{origin}/artifact"}}]}}}}"#
            );
            return ("200 OK".to_string(), body.into_bytes());
        }
        if path == "/artifact" {
            return ("200 OK".to_string(), artifact.clone());
        }
        ("404 Not Found".to_string(), b"nope".to_vec())
    });

    let root = tempfile::tempdir().expect("temp dir");
    let dir = root.path().join("out");
    let cfg = UpgradeConfig {
        dir: dir.to_string_lossy().to_string(),
        verify: true,
        ..config_for(&server.base_url, "v9.9.9")
    };
    let code = retry_busy(|| run(cfg.clone())).expect("upgrade succeeds");
    assert_eq!(code, 0);

    let installed = dir.join(format!("devine-utils-ci-{}-{}", go_os(), go_arch()));
    assert!(installed.exists(), "{} missing", installed.display());
    let mode = std::fs::metadata(&installed)
        .expect("installed file")
        .permissions()
        .mode();
    assert_ne!(mode & 0o100, 0, "installed binary is not executable");
    server.shutdown();
}

mod release;
