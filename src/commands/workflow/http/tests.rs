// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the two polls, against loopback servers that answer the way
//! artifactory and a FAAS `/version` endpoint do.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;

/// A loopback HTTP server that always answers the same status and body.
struct Server {
    base: String,
    hits: Arc<AtomicUsize>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    /// Serves `status` + `body` on every path.
    fn start(status: u16, body: &'static str) -> Server {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback bind");
        listener
            .set_nonblocking(true)
            .expect("non-blocking listener");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handle = std::thread::spawn(move || loop {
            let accepted = match listener.accept() {
                Ok((stream, _)) => Some(stream),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
                Err(_) => break,
            };
            match accepted {
                Some(mut stream) => {
                    counter.fetch_add(1, Ordering::SeqCst);
                    let mut buffer = [0u8; 2048];
                    let _ = stream.read(&mut buffer);
                    let response = format!(
                        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                }
                None => {
                    if flag.load(Ordering::SeqCst) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        });
        Server {
            base,
            hits,
            stop,
            handle: Some(handle),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[test]
fn probe_app_version_reads_app_version() {
    let server = Server::start(200, r#"{"app_version":"0.1.35-rc.2","commit":"abc"}"#);
    let probe = probe_app_version(&server.url("/version"));
    assert_eq!(probe.status, 200);
    assert_eq!(probe.got, "0.1.35-rc.2");
    assert!(probe.error.is_none());
}

#[test]
fn probe_app_version_reports_unparseable_bodies() {
    let server = Server::start(200, "not json");
    let probe = probe_app_version(&server.url("/version"));
    let error = probe.error.expect("parse error");
    assert!(error.starts_with("parse /version "), "{error}");
    assert!(error.contains("body: not json"), "{error}");
}

#[test]
fn probe_app_version_reports_a_transport_failure() {
    // Nothing listens on port 1, so the connection is refused.
    let probe = probe_app_version("http://127.0.0.1:1/version");
    assert_eq!(probe.status, 0);
    assert!(probe.got.is_empty());
    assert!(probe.error.is_some());
}

#[test]
fn poll_app_version_returns_the_first_match() {
    let server = Server::start(200, r#"{"app_version":"1.2.3-rc.1"}"#);
    let got = poll_app_version(
        &server.url("/version"),
        "1.2.3-rc.1",
        5,
        false,
        Duration::from_millis(10),
    )
    .unwrap();
    assert_eq!(got, "1.2.3-rc.1");
    assert_eq!(server.hits(), 1);
}

#[test]
fn poll_app_version_accepts_any_version_when_want_is_empty() {
    let server = Server::start(200, r#"{"app_version":"9.9.9"}"#);
    let got = poll_app_version(
        &server.url("/version"),
        "",
        5,
        false,
        Duration::from_millis(10),
    )
    .unwrap();
    assert_eq!(got, "9.9.9");
}

#[test]
fn poll_app_version_blocks_on_a_mismatch() {
    let server = Server::start(200, r#"{"app_version":"0.1.18"}"#);
    let error = poll_app_version(
        &server.url("/version"),
        "0.1.19-rc.1",
        1,
        false,
        Duration::from_millis(20),
    )
    .unwrap_err();
    assert!(error.contains("never reached \"0.1.19-rc.1\""), "{error}");
    assert!(server.hits() > 1, "it must keep polling");
}

#[test]
fn head_reports_the_content_length() {
    let server = Server::start(200, "0123456789");
    let info = http_head(&server.url("/binary"));
    assert_eq!(info.code, 200);
    assert_eq!(info.len, 10);
    assert!(info.present);
}

#[test]
fn poll_artifactory_accepts_a_large_enough_artifact() {
    let big: &'static str = Box::leak("x".repeat(2048).into_boxed_str());
    let server = Server::start(200, big);
    // The body is 2048 bytes, so the HEAD answers Content-Length: 2048.
    let info = poll_artifactory(
        "UAT",
        &server.url("/binary"),
        1024,
        5,
        false,
        Duration::from_millis(10),
    )
    .unwrap();
    assert!(info.len >= 1024);
    assert!(info.present);
}

#[test]
fn poll_artifactory_times_out_with_the_exact_reason() {
    let server = Server::start(404, "missing");
    let error = poll_artifactory(
        "PROD",
        &server.url("/binary"),
        1024,
        0,
        false,
        Duration::from_millis(10),
    )
    .unwrap_err();
    assert!(
        error.starts_with("PROD artifact not live after 0s"),
        "{error}"
    );
    assert!(error.contains("HTTP 404"), "{error}");
    assert!(error.contains("need 200, >=1024 B"), "{error}");
}
