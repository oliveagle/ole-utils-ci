// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The promote fixture: a binary repo on main with a bare origin, plus a
//! loopback service answering `/version` and the PROD artifact HEAD.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;

/// Runs `git <args>` in `dir`, asserting success.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A binary repo on main, published to a bare origin.
pub struct Fixture {
    pub root: TempDir,
    bare: String,
    pub local: std::path::PathBuf,
}

impl Fixture {
    /// Creates the repo holding `version` on main.
    pub fn new(version: &str) -> Fixture {
        let root = TempDir::new().unwrap();
        let bare = root.path().join("test-binary.git");
        let local = root.path().join("local");
        git(
            root.path(),
            &[
                "init",
                "--bare",
                "--initial-branch=main",
                bare.to_str().unwrap(),
            ],
        );
        std::fs::create_dir_all(&local).unwrap();
        git(&local, &["init", "--initial-branch=main"]);
        git(&local, &["config", "user.name", "Workflow Test"]);
        git(
            &local,
            &["config", "user.email", "workflow-test@example.invalid"],
        );
        std::fs::write(local.join("VERSION"), format!("{version}\n")).unwrap();
        std::fs::write(
            local.join("CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n\n- pending\n",
        )
        .unwrap();
        git(&local, &["add", "VERSION", "CHANGELOG.md"]);
        git(&local, &["commit", "-m", "initial"]);
        git(&local, &["remote", "add", "origin", bare.to_str().unwrap()]);
        git(&local, &["push", "-u", "origin", "main"]);
        Fixture {
            root,
            bare: bare.display().to_string(),
            local,
        }
    }

    /// The version on the bare origin's main.
    pub fn bare_version(&self) -> String {
        let output = std::process::Command::new("git")
            .args(["--git-dir", &self.bare, "show", "main:VERSION"])
            .output()
            .expect("run git");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// The local tags, one per line.
    pub fn tags(&self) -> String {
        git(&self.local, &["tag", "--list"])
    }
}

/// Serves `/version` with `app_version`, and counts PROD artifact probes.
pub struct Services {
    pub base: String,
    pub version_hits: Arc<AtomicUsize>,
    pub prod_hits: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Services {
    /// Starts the service answering `app_version` on `/version`.
    pub fn start(app_version: &'static str) -> Services {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback bind");
        listener.set_nonblocking(true).expect("non-blocking");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let version_hits = Arc::new(AtomicUsize::new(0));
        let prod_hits = Arc::new(AtomicUsize::new(0));
        let versions = Arc::clone(&version_hits);
        let prods = Arc::clone(&prod_hits);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handle = std::thread::spawn(move || loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0u8; 2048];
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                    let (status, body) = if request.starts_with("GET /version") {
                        versions.fetch_add(1, Ordering::SeqCst);
                        (200, format!(r#"{{"app_version":"{app_version}"}}"#))
                    } else if request.contains("/prod/") {
                        prods.fetch_add(1, Ordering::SeqCst);
                        (200, "data".to_string())
                    } else {
                        (404, "missing".to_string())
                    };
                    let response = format!(
                        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if flag.load(Ordering::SeqCst) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(_) => break,
            }
        });
        Services {
            base,
            version_hits,
            prod_hits,
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for Services {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
