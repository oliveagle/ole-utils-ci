// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for `workflow prod-verify`: the stable-only gate, the version
//! resolution order and the artifact wait.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::commands::workflow::defaults::default_workflow_config;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

/// A poll step short enough to keep the suite fast.
fn fast() -> Duration {
    Duration::from_millis(1)
}

/// A loopback artifact server answering one fixed status and body.
struct Artifactory {
    base: String,
    hits: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Artifactory {
    fn start(status: u16, body: &'static str) -> Artifactory {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback bind");
        listener.set_nonblocking(true).expect("non-blocking");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handle = std::thread::spawn(move || loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    counter.fetch_add(1, Ordering::SeqCst);
                    let mut buffer = [0u8; 2048];
                    let _ = stream.read(&mut buffer);
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
        Artifactory {
            base,
            hits,
            stop,
            handle: Some(handle),
        }
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

impl Drop for Artifactory {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// A workflow whose artifact base points at `base`.
fn config(base: &str) -> WorkflowConfig {
    let mut cfg = default_workflow_config();
    cfg.artifactory.base_url = base.to_string();
    cfg.artifactory.min_size_b = 1;
    cfg
}

/// An empty context, so no version file is picked up from the working dir.
fn ctx() -> Context {
    Context {
        cwd: std::path::PathBuf::from("/nonexistent"),
        home: String::new(),
    }
}

/// Parses the flags the way the command does.
fn flags(values: &[&str]) -> (ProdVerifyFlags, Vec<String>) {
    parse_prod_verify(&args(values))
}

#[test]
fn the_version_flag_beats_from_and_positional() {
    let cfg = config("http://x");
    let (f, positional) = flags(&["--version", "1.2.3", "--from", "9.9.9", "8.8.8"]);
    assert_eq!(
        resolve_version(&f, &positional, &cfg, &ctx()).unwrap(),
        "1.2.3"
    );
}

#[test]
fn from_beats_the_positional_and_a_blank_version_falls_through() {
    let cfg = config("http://x");
    let (f, positional) = flags(&["--from", "9.9.9", "8.8.8"]);
    assert_eq!(
        resolve_version(&f, &positional, &cfg, &ctx()).unwrap(),
        "9.9.9"
    );

    let (f, positional) = flags(&["--version", "   ", "8.8.8"]);
    assert_eq!(
        resolve_version(&f, &positional, &cfg, &ctx()).unwrap(),
        "8.8.8"
    );
}

#[test]
fn a_positional_version_is_trimmed() {
    let cfg = config("http://x");
    let (f, _) = flags(&[]);
    assert_eq!(
        resolve_version(&f, &args(&[" 0.1.24 "]), &cfg, &ctx()).unwrap(),
        "0.1.24"
    );
}

#[test]
fn no_version_anywhere_is_an_error_that_says_what_to_pass() {
    let cfg = config("http://x");
    let (f, _) = flags(&[]);
    let error = resolve_version(&f, &[], &cfg, &ctx()).unwrap_err();
    assert!(
        error.contains("cannot determine the stable version"),
        "{error}"
    );
    assert!(error.contains("--from <x.y.z>"), "{error}");
}

#[test]
fn more_than_one_positional_is_rejected() {
    let cfg = default_workflow_config();
    let error = workflow_prod_verify(&args(&["1.2.3", "1.2.4"]), &cfg, &ctx()).unwrap_err();
    assert!(
        error.contains("at most one positional <version>"),
        "{error}"
    );
    assert!(error.contains("[1.2.3 1.2.4]"), "{error}");
}

#[test]
fn an_rc_is_refused_before_anything_is_triggered() {
    let cfg = default_workflow_config();
    let error = workflow_prod_verify(&args(&["0.1.25-rc.6"]), &cfg, &ctx()).unwrap_err();
    assert!(error.contains("REFUSED"), "{error}");
    assert!(error.contains("is not a stable release (x.y.z)"), "{error}");
    assert!(error.contains("workflow release-rc / test-rc"), "{error}");
}

#[test]
fn a_stable_version_dry_runs_without_touching_the_network() {
    let server = Artifactory::start(500, "");
    let cfg = config(&server.base);
    assert_eq!(
        workflow_prod_verify(&args(&["0.1.24", "--dry-run"]), &cfg, &ctx()).unwrap(),
        0
    );
    assert_eq!(server.hits(), 0, "a dry run must not poll artifactory");
}

#[test]
fn a_filter_matching_nothing_is_an_error() {
    let cfg = default_workflow_config();
    let error = workflow_prod_verify(
        &args(&["0.1.24", "--dry-run", "--apps", "nope"]),
        &cfg,
        &ctx(),
    )
    .unwrap_err();
    assert!(error.contains("no test apps selected"), "{error}");
}

#[test]
fn the_artifact_wait_can_be_skipped() {
    let cfg = config("http://127.0.0.1:1");
    let (f, _) = flags(&["0.1.24", "--skip-artifact"]);
    check_artifact(&cfg, "0.1.24", &f, fast()).expect("skipped");
}

#[test]
fn the_artifact_wait_accepts_a_live_artifact() {
    let server = Artifactory::start(200, "0123456789");
    let cfg = config(&server.base);
    let (f, _) = flags(&["0.1.24"]);
    check_artifact(&cfg, "0.1.24", &f, fast()).expect("artifact is live");
    assert!(server.hits() >= 1);
}

#[test]
fn a_missing_artifact_points_at_the_manual_publish_step() {
    let server = Artifactory::start(404, "missing");
    let cfg = config(&server.base);
    let (mut f, _) = flags(&["0.1.24"]);
    f.artifact_timeout = 1;
    let error = check_artifact(&cfg, "0.1.24", &f, fast()).unwrap_err();
    assert!(error.contains("prod-verify artifact not live"), "{error}");
    assert!(
        error.contains("did the owner click ▶ Upload PROD?"),
        "{error}"
    );
}
