// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the two release rules. The gate is the one thing that must
//! never fail open, so both the refusal paths and the exact wording are pinned.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::commands::workflow::defaults::default_workflow_config;

/// A loopback `/version` server with a fixed answer.
struct VersionServer {
    base: String,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl VersionServer {
    fn start(status: u16, body: &'static str) -> VersionServer {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback bind");
        listener.set_nonblocking(true).expect("non-blocking");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handle = std::thread::spawn(move || loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
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
        VersionServer {
            base,
            stop,
            handle: Some(handle),
        }
    }

    fn url(&self) -> String {
        format!("{}/version", self.base)
    }
}

impl Drop for VersionServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The built-in catalog with integration-01's `/version` pointed at `url`.
fn config_pointing_at(url: &str) -> WorkflowConfig {
    let mut cfg = default_workflow_config();
    for app in &mut cfg.test_apps {
        if app.name == GATE_APP_NAME {
            app.version_url = url.to_string();
        }
    }
    cfg
}

#[test]
fn the_gate_passes_when_integration01_serves_the_rc() {
    let server = VersionServer::start(200, r#"{"app_version":"0.1.19-rc.1"}"#);
    let cfg = config_pointing_at(&server.url());
    gate_check_integration01_before_promote(&cfg, "0.1.19-rc.1").expect("gate passes");
}

#[test]
fn the_gate_blocks_an_older_version_and_quotes_the_rule() {
    let server = VersionServer::start(200, r#"{"app_version":"0.1.18"}"#);
    let cfg = config_pointing_at(&server.url());
    let error = gate_check_integration01_before_promote(&cfg, "0.1.19-rc.1").unwrap_err();
    assert!(error.contains("release gate: BLOCKED"), "{error}");
    assert!(
        error.contains("is serving app_version=\"0.1.18\""),
        "{error}"
    );
    assert!(
        error.contains("you are promoting \"0.1.19-rc.1\""),
        "{error}"
    );
    assert!(
        error.contains("没有完成集成测试 01 的生产发布测试的，不允许发布正式版本."),
        "{error}"
    );
    assert!(
        error.contains("Fix:  devine-utils-ci workflow integration-rc --app"),
        "{error}"
    );
}

#[test]
fn the_gate_blocks_a_non_200_answer_even_when_the_body_matches() {
    let server = VersionServer::start(503, r#"{"app_version":"0.1.19-rc.1"}"#);
    let cfg = config_pointing_at(&server.url());
    let error = gate_check_integration01_before_promote(&cfg, "0.1.19-rc.1").unwrap_err();
    assert!(error.contains("returned HTTP 503 (want 200)"), "{error}");
    assert!(error.contains("the app is not healthy"), "{error}");
}

#[test]
fn the_gate_blocks_an_unreachable_version_endpoint() {
    let cfg = config_pointing_at("http://127.0.0.1:1/version");
    let error = gate_check_integration01_before_promote(&cfg, "0.1.19-rc.1").unwrap_err();
    assert!(
        error.contains("could not reach devine-app-integration-01-function"),
        "{error}"
    );
    assert!(
        error.contains("cannot verify integration-01 is serving"),
        "{error}"
    );
}

#[test]
fn the_gate_refuses_an_empty_want_before_probing() {
    let cfg = default_workflow_config();
    let error = gate_check_integration01_before_promote(&cfg, "").unwrap_err();
    assert!(
        error.contains("cannot determine the rc to promote"),
        "{error}"
    );
}

#[test]
fn the_gate_is_pinned_to_integration01_not_to_a_role() {
    let mut cfg = default_workflow_config();
    cfg.test_apps.retain(|app| app.name != GATE_APP_NAME);
    let error = gate_check_integration01_before_promote(&cfg, "0.1.19-rc.1").unwrap_err();
    assert!(
        error.contains("integration-01 app not found in catalog"),
        "{error}"
    );
}

#[test]
fn the_post_promote_rule_names_integration02() {
    let mut cfg = default_workflow_config();
    cfg.test_apps
        .retain(|app| app.name != POST_PROMOTE_APP_NAME);
    let error = run_integration02_after_promote(&cfg, "0.1.19", 5, false).unwrap_err();
    assert!(
        error.contains("integration-02 app not found in catalog"),
        "{error}"
    );
}

#[test]
fn want_version_is_empty_when_the_binary_repo_is_not_visible() {
    let mut cfg = default_workflow_config();
    cfg.binary_repo.version_file = "/definitely/not/a/path/VERSION".to_string();
    let ctx = Context {
        cwd: std::path::PathBuf::from("/definitely/not/a/repo"),
        home: String::new(),
    };
    assert_eq!(workflow_want_version(&cfg, &ctx), "");
}

#[test]
fn want_version_reads_the_checkout_in_the_working_directory() {
    let dir = tempfile::TempDir::new().unwrap();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .output()
            .expect("run git");
        assert!(output.status.success(), "git {args:?}");
    };
    git(&["init", "--initial-branch=main"]);
    git(&[
        "remote",
        "add",
        "origin",
        "https://git.example/devine/devine-golang-mcp-app.git",
    ]);
    std::fs::write(dir.path().join("VERSION"), "0.1.35-rc.2\n").unwrap();
    let cfg = default_workflow_config();
    let ctx = Context {
        cwd: dir.path().to_path_buf(),
        home: String::new(),
    };
    assert_eq!(workflow_want_version(&cfg, &ctx), "0.1.35-rc.2");
}
