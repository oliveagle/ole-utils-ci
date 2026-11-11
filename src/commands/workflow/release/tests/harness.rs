// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The release-rc fixture: a binary repo on main with a bare origin, plus a
//! loopback artifactory that counts the UAT and PROD probes.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;

use crate::commands::workflow::model::{Artifactory, BinaryRepo, WorkflowConfig};
use crate::commands::workflow::registry::Context;

/// Runs `git <args>` in `dir`, asserting success.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} in {} failed: {}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn configure(dir: &Path) {
    git(dir, &["config", "user.name", "Workflow Test"]);
    git(
        dir,
        &["config", "user.email", "workflow-test@example.invalid"],
    );
}

/// A binary repo with a bare origin, and the local checkout on `main`.
pub struct Fixture {
    pub root: TempDir,
    pub bare: String,
    pub local: std::path::PathBuf,
    pub updater: std::path::PathBuf,
}

impl Fixture {
    /// `version` is what `main` holds on the remote after the updater commits.
    pub fn new(version: &str) -> Fixture {
        let root = TempDir::new().unwrap();
        let bare = root.path().join("test-binary.git");
        let local = root.path().join("local");
        let updater = root.path().join("updater");
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
        configure(&local);
        std::fs::write(local.join("VERSION"), "0.1.34-rc.1\n").unwrap();
        std::fs::write(
            local.join("CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n\n- pending\n",
        )
        .unwrap();
        git(&local, &["add", "VERSION", "CHANGELOG.md"]);
        git(&local, &["commit", "-m", "initial"]);
        git(&local, &["remote", "add", "origin", bare.to_str().unwrap()]);
        git(&local, &["push", "-u", "origin", "main"]);

        // A stale feature branch, so the command must switch to main first.
        git(&local, &["checkout", "-b", "feature/stale-version"]);
        std::fs::write(local.join("VERSION"), "9.8.7-rc.3\n").unwrap();
        git(&local, &["add", "VERSION"]);
        git(&local, &["commit", "-m", "stale feature version"]);

        git(
            root.path(),
            &["clone", bare.to_str().unwrap(), updater.to_str().unwrap()],
        );
        configure(&updater);
        std::fs::write(updater.join("VERSION"), format!("{version}\n")).unwrap();
        git(&updater, &["add", "VERSION"]);
        git(&updater, &["commit", "-m", "advance main"]);
        git(&updater, &["push", "origin", "main"]);

        Fixture {
            root,
            bare: bare.display().to_string(),
            local,
            updater,
        }
    }

    /// The VERSION file `main` holds on the bare remote.
    pub fn bare_version(&self) -> String {
        let output = std::process::Command::new("git")
            .args(["--git-dir", &self.bare, "show", "main:VERSION"])
            .output()
            .expect("run git");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }
}

/// An artifactory stand-in that counts UAT and PROD probes.
pub struct ArtifactoryServer {
    pub base: String,
    pub uat: Arc<AtomicUsize>,
    pub prod: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl ArtifactoryServer {
    pub fn start() -> ArtifactoryServer {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback bind");
        listener.set_nonblocking(true).expect("non-blocking");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let uat = Arc::new(AtomicUsize::new(0));
        let prod = Arc::new(AtomicUsize::new(0));
        let uat_hits = Arc::clone(&uat);
        let prod_hits = Arc::clone(&prod);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handle = std::thread::spawn(move || loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0u8; 2048];
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                    if request.contains("/uat/") {
                        uat_hits.fetch_add(1, Ordering::SeqCst);
                    }
                    if request.contains("/prod/") {
                        prod_hits.fetch_add(1, Ordering::SeqCst);
                    }
                    let body = "data";
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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
        ArtifactoryServer {
            base,
            uat,
            prod,
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for ArtifactoryServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The built-in topology, pointed at `base`.
pub fn config(base: &str) -> WorkflowConfig {
    WorkflowConfig {
        name: "test-binary".to_string(),
        binary_repo: BinaryRepo {
            name: "test-binary".to_string(),
            project_path: "test/test-binary".to_string(),
            gitlab_base: "https://git.example/test/test-binary".to_string(),
            version_file: "VERSION".to_string(),
            changelog_file: "CHANGELOG.md".to_string(),
            mirror_files: Vec::new(),
            remote: "origin".to_string(),
        },
        artifactory: Artifactory {
            base_url: base.to_string(),
            uat_path: "repo/uat".to_string(),
            prod_path: "repo/prod".to_string(),
            min_size_b: 4,
        },
        test_apps: Vec::new(),
    }
}

/// A context rooted at `dir`.
pub fn ctx(dir: &Path) -> Context {
    Context {
        cwd: dir.to_path_buf(),
        home: String::new(),
    }
}
