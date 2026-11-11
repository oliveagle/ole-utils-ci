// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for resolving an app straight from the FAAS registry: the two
//! fail-closed restrictions and the ksvc -> release-group mapping.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;

/// A FAAS stand-in answering the two read endpoints from a route table.
struct FaasServer {
    base: String,
    stop: Arc<AtomicBool>,
    paths: Arc<std::sync::Mutex<Vec<String>>>,
    auth: Arc<std::sync::Mutex<Vec<String>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FaasServer {
    fn start(function: String, ksvcs: String) -> FaasServer {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback bind");
        listener.set_nonblocking(true).expect("non-blocking");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let paths = Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = Arc::clone(&paths);
        let auth = Arc::new(std::sync::Mutex::new(Vec::new()));
        let auth_log = Arc::clone(&auth);
        let handle = std::thread::spawn(move || loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0u8; 4096];
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or("")
                        .to_string();
                    let header = request
                        .lines()
                        .find(|line| line.to_lowercase().starts_with("authorization:"))
                        .unwrap_or("")
                        .to_string();
                    if let Ok(mut slot) = log.lock() {
                        slot.push(path.clone());
                    }
                    if let Ok(mut slot) = auth_log.lock() {
                        slot.push(header);
                    }
                    let body = if path.starts_with("/functions/") && path.ends_with("/ksvcs") {
                        &ksvcs
                    } else {
                        &function
                    };
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
        FaasServer {
            base,
            stop,
            paths,
            auth,
            handle: Some(handle),
        }
    }

    fn paths(&self) -> Vec<String> {
        self.paths
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default()
    }

    fn auth(&self) -> Vec<String> {
        self.auth
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default()
    }
}

impl Drop for FaasServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// A `GolangDevine` function with `level` importance.
fn function(name: &str, language: &str, code: &str, level: &str) -> String {
    format!(
        r#"{{"count":1,"result":[{{"id":42,"name":"{name}","language":"{language}",
        "repo":"faas/{name}","importance_level":"{level}",
        "pipeline_template":{{"pipeline_code":"{code}","name":"{code}"}}}}]}}"#
    )
}

#[test]
fn repo_path_normalisation() {
    assert_eq!(faas_repo_path("dg-function"), "faas/dg-function");
    assert_eq!(faas_repo_path("dg"), "faas/dg-function");
    assert_eq!(faas_repo_path("dg.git"), "faas/dg-function");
    assert_eq!(faas_repo_path("faas/dg-function"), "faas/dg-function");
    assert_eq!(faas_repo_path("faas/dg-function.git"), "faas/dg-function");
    assert_eq!(faas_repo_path(" git@h:g/p.git "), "git@h:g/p");
}

#[test]
fn a_devine_golang_app_maps_ksvcs_onto_the_release_topology() {
    let ksvcs = r#"{"dg-function":[
        {"id":11,"env":"fat","sub_env":"fws","region":"NTG","idc":"NTGXH","url":"http://dg-function.fws.example"},
        {"id":27819,"group_id":1044275,"env":"prod","region":"SHA","idc":"SHAXY","url":"https://dg-function.example"},
        {"id":27820,"group_id":1044277,"env":"prod","region":"SHARB","idc":"SHARB","url":"https://dg-function.example"}]}"#;
    let server = FaasServer::start(
        function("dg-function", "golang", "GolangDevineMCPFunc", "L5"),
        ksvcs.to_string(),
    );
    let app = resolve_faas_app(&server.base, "tok", "dg-function").unwrap();
    assert_eq!(app.name, "dg-function");
    assert_eq!(app.project_path, "faas/dg-function");
    assert_eq!(app.role, "faas-devine-golang");
    assert_eq!(app.fat_group, 11, "the FAT ksvc is the image source");
    assert_eq!(app.version_url, "http://dg-function.fws.example/version");
    assert_eq!(app.prod_url, "https://dg-function.example");
    assert_eq!(app.prod_groups.len(), 2);
    // The release group id wins over the ksvc id: the release API keys on it.
    assert_eq!(app.prod_groups[0].id, 1_044_275);
    assert_eq!(app.prod_groups[0].name, "SHAXY");
    assert!(app.prod_groups[0].enabled);
    assert_eq!(app.prod_groups[1].id, 1_044_277);
    // The repo filter and the auth headers are what FAAS expects.
    assert_eq!(server.paths()[0], "/functions?repo=faas/dg-function");
    assert_eq!(server.paths()[1], "/functions/42/ksvcs");
    assert!(server.auth()[0]
        .to_lowercase()
        .starts_with("authorization: bearer tok"));
}

#[test]
fn a_non_golang_function_is_refused() {
    let server = FaasServer::start(
        function("x-function", "nodejs", "NodeJSServerlessSTD", "L5"),
        "{}".to_string(),
    );
    let error = resolve_faas_app(&server.base, "tok", "faas/x-function").unwrap_err();
    assert!(
        error.contains("only FAAS devine-golang apps are supported"),
        "{error}"
    );
    assert!(error.contains("GolangDevine"), "{error}");
}

#[test]
fn a_core_app_is_refused_and_an_absent_level_too() {
    for level in ["L1", "L2", "L3", ""] {
        let server = FaasServer::start(
            function("core-function", "golang", "GolangDevineMCPFunc", level),
            "{}".to_string(),
        );
        let error = resolve_faas_app(&server.base, "tok", "faas/core-function").unwrap_err();
        assert!(
            error.contains("only NON-CORE FAAS apps are supported"),
            "{level:?}: {error}"
        );
    }
}

#[test]
fn l4_and_l5_are_non_core_but_case_insensitive() {
    for level in ["L4", " l4 "] {
        let ksvcs = r#"{"dg-function":[{"id":11,"env":"fat","url":"http://x"},
            {"id":22,"group_id":1044275,"env":"prod","idc":"SHAXY","url":"https://x"}]}"#;
        let server = FaasServer::start(
            function("dg-function", "golang", "GolangDevineMCPFunc", level),
            ksvcs.to_string(),
        );
        resolve_faas_app(&server.base, "tok", "dg-function").expect(level);
    }
}

#[test]
fn an_app_without_prod_ksvcs_is_refused_not_silently_skipped() {
    let server = FaasServer::start(
        function("dg-function", "golang", "GolangDevineMCPFunc", "L5"),
        r#"{"dg-function":[{"id":11,"env":"fat","url":"http://x"}]}"#.to_string(),
    );
    let error = resolve_faas_app(&server.base, "tok", "dg-function").unwrap_err();
    assert!(error.contains("no PROD ksvc/groups provisioned"), "{error}");
}

#[test]
fn an_app_without_a_fat_ksvc_is_refused() {
    let server = FaasServer::start(
        function("dg-function", "golang", "GolangDevineMCPFunc", "L5"),
        r#"{"dg-function":[{"id":22,"env":"prod","idc":"SHAXY","url":"https://x"}]}"#.to_string(),
    );
    let error = resolve_faas_app(&server.base, "tok", "dg-function").unwrap_err();
    assert!(error.contains("has no fat ksvc"), "{error}");
}

#[test]
fn deleted_ksvcs_are_ignored() {
    let ksvcs = r#"{"dg-function":[
        {"id":11,"env":"fat","url":"http://x"},
        {"id":12,"env":"fat","url":"http://y","deleted_at":"2026-01-01"},
        {"id":22,"env":"prod","idc":"SHAXY","url":"https://x"},
        {"id":23,"env":"prod","idc":"SHARB","url":"https://y","deleted_at":"2026-01-01"}]}"#;
    let server = FaasServer::start(
        function("dg-function", "golang", "GolangDevineMCPFunc", "L5"),
        ksvcs.to_string(),
    );
    let app = resolve_faas_app(&server.base, "tok", "dg-function").unwrap();
    assert_eq!(app.fat_group, 11, "the first live FAT ksvc wins");
    assert_eq!(app.prod_groups.len(), 1);
    assert_eq!(app.prod_groups[0].id, 22);
}

#[test]
fn a_function_name_keyed_differently_still_resolves() {
    let server = FaasServer::start(
        function("dg-function", "golang", "GolangDevineMCPFunc", "L5"),
        r#"{"other-key":[{"id":11,"env":"fat","url":"http://x"},
            {"id":22,"env":"prod","idc":"SHAXY","url":"https://x"}]}"#
            .to_string(),
    );
    let app = resolve_faas_app(&server.base, "tok", "dg-function").unwrap();
    assert_eq!(app.fat_group, 11);
    assert_eq!(app.prod_groups.len(), 1);
}

#[test]
fn an_unknown_repo_says_what_to_pass_instead() {
    let server = FaasServer::start(r#"{"count":0,"result":[]}"#.to_string(), "{}".to_string());
    let error = resolve_faas_app(&server.base, "tok", "nothing").unwrap_err();
    assert!(
        error.contains("no FAAS function for repo \"faas/nothing-function\""),
        "{error}"
    );
    assert!(error.contains("pass --app"), "{error}");
}

#[test]
fn a_failed_read_reports_the_status_and_body() {
    // Nothing listens on port 1, so the GET cannot even connect.
    let error = resolve_faas_app("http://127.0.0.1:1", "tok", "dg-function").unwrap_err();
    assert!(error.contains("look up FAAS function"), "{error}");
}
