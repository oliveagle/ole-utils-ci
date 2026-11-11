// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for one MCP `tools/call` exchange: every verdict an app can
//! produce is normalised here, so each one is pinned.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;

/// A loopback `/mcp` server that always answers the same status and body.
struct McpServer {
    base: String,
    stop: Arc<AtomicBool>,
    seen: Arc<std::sync::Mutex<String>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl McpServer {
    fn start(status: u16, body: &'static str) -> McpServer {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback bind");
        listener.set_nonblocking(true).expect("non-blocking");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let seen = Arc::new(std::sync::Mutex::new(String::new()));
        let log = Arc::clone(&seen);
        let handle = std::thread::spawn(move || loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0u8; 8192];
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    if let Ok(mut slot) = log.lock() {
                        *slot = String::from_utf8_lossy(&buffer[..read]).to_string();
                    }
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
        McpServer {
            base,
            stop,
            seen,
            handle: Some(handle),
        }
    }

    fn request(&self) -> String {
        self.seen
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default()
    }
}

impl Drop for McpServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// A JSON-RPC envelope whose payload is `payload`.
fn envelope(payload: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":1,"result":{{"content":[{{"type":"text","text":{}}}]}}}}"#,
        serde_json::to_string(payload).unwrap()
    )
}

#[test]
fn strip_sse_unwraps_a_single_data_frame() {
    assert_eq!(strip_sse(r#"{"a":1}"#), r#"{"a":1}"#);
    assert_eq!(
        strip_sse("event: message\ndata: {\"a\":1}\n\n"),
        r#"{"a":1}"#
    );
}

#[test]
fn a_passing_payload_is_reported_with_its_details() {
    let body: &'static str = Box::leak(
        envelope(r#"{"verdict":"PASS","environment":"FAT","selfIdc":"NTGXH","peerIdc":"SHAXY"}"#)
            .into_boxed_str(),
    );
    let server = McpServer::start(200, body);
    let result = run_scenario_tool("fat", "app", &server.base, "it_downstream_routing");
    assert_eq!(result.verdict, "PASS");
    assert_eq!(result.environment, "FAT");
    assert_eq!(result.self_idc, "NTGXH");
    assert_eq!(result.peer_idc, "SHAXY");
    assert_eq!(result.tool, "it_downstream_routing");
    assert_eq!(result.env, "fat");
    // The request is a tools/call naming the scenario.
    let request = server.request();
    assert!(request.contains("\"method\":\"tools/call\""), "{request}");
    assert!(
        request.contains("\"name\":\"it_downstream_routing\""),
        "{request}"
    );
}

#[test]
fn a_failing_payload_carries_the_failed_check() {
    let body: &'static str = Box::leak(
        envelope(r#"{"verdict":"FAIL","failedCheck":"peer unreachable"}"#).into_boxed_str(),
    );
    let server = McpServer::start(200, body);
    let result = run_scenario_tool("prod", "app", &server.base, "it_mcp_chain");
    assert_eq!(result.verdict, "FAIL");
    assert_eq!(result.detail, "peer unreachable");
}

#[test]
fn a_tool_the_app_does_not_ship_is_missing() {
    let body: &'static str = Box::leak(
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"tool 'it_x' not found: tool not found"}}"#
            .to_string()
            .into_boxed_str(),
    );
    let server = McpServer::start(200, body);
    let result = run_scenario_tool("fat", "app", &server.base, "it_x");
    assert_eq!(result.verdict, "MISSING");
    assert!(result.detail.contains("not found"));
}

#[test]
fn any_other_json_rpc_error_is_an_error() {
    let body: &'static str = Box::leak(
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"boom"}}"#
            .to_string()
            .into_boxed_str(),
    );
    let server = McpServer::start(200, body);
    assert_eq!(
        run_scenario_tool("fat", "app", &server.base, "it_x").verdict,
        "ERROR"
    );
}

#[test]
fn transport_and_shape_failures_are_errors() {
    let unreachable = run_scenario_tool("fat", "app", "http://127.0.0.1:1", "it_x");
    assert_eq!(unreachable.verdict, "ERROR");

    let http_error = McpServer::start(500, "{}");
    let result = run_scenario_tool("fat", "app", &http_error.base, "it_x");
    assert_eq!(result.verdict, "ERROR");
    assert_eq!(result.detail, "HTTP 500");

    let unparseable = McpServer::start(200, "not json");
    let result = run_scenario_tool("fat", "app", &unparseable.base, "it_x");
    assert_eq!(result.detail, "unparseable JSON-RPC response");

    let empty: &'static str = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[]}}"#;
    let empty = McpServer::start(200, empty);
    assert_eq!(
        run_scenario_tool("fat", "app", &empty.base, "it_x").detail,
        "empty result"
    );

    let no_result: &'static str = r#"{"jsonrpc":"2.0","id":1}"#;
    let no_result = McpServer::start(200, no_result);
    assert_eq!(
        run_scenario_tool("fat", "app", &no_result.base, "it_x").detail,
        "empty result"
    );

    let body: &'static str = Box::leak(envelope("not json at all").into_boxed_str());
    let bad_payload = McpServer::start(200, body);
    assert_eq!(
        run_scenario_tool("fat", "app", &bad_payload.base, "it_x").detail,
        "unparseable tool payload"
    );
}

#[test]
fn an_unknown_verdict_is_an_error() {
    let body: &'static str = Box::leak(envelope(r#"{"verdict":"MAYBE"}"#).into_boxed_str());
    let server = McpServer::start(200, body);
    assert_eq!(
        run_scenario_tool("fat", "app", &server.base, "it_x").verdict,
        "ERROR"
    );
}
