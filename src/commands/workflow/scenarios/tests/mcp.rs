// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! A fake scenario app: JSON-RPC over `POST /mcp`, one entry per tool.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One fake FaaS app answering `/mcp` for the tools it was given.
pub struct McpApp {
    pub base: String,
    hits: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl McpApp {
    /// Serves `verdicts` (tool name -> verdict); any other tool answers the
    /// JSON-RPC "not found" error an app without the scenario tools returns.
    pub fn start(verdicts: Vec<(&'static str, &'static str)>) -> McpApp {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback bind");
        listener.set_nonblocking(true).expect("non-blocking");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let table = Arc::new(Mutex::new(verdicts));
        let handle = std::thread::spawn(move || loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    counter.fetch_add(1, Ordering::SeqCst);
                    let request = read_request(&mut stream);
                    let body = answer(&table, &request);
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
        McpApp {
            base,
            hits,
            stop,
            handle: Some(handle),
        }
    }

    /// How many requests the app saw.
    pub fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

impl Drop for McpApp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Reads one request, body included.
fn read_request(stream: &mut std::net::TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");
    let mut raw = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        if let Some(index) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&raw[..index]).to_lowercase();
            let wanted: usize = head
                .split("\r\n")
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.trim() == "content-length")
                .and_then(|(_, value)| value.trim().parse().ok())
                .unwrap_or(0);
            if raw.len() >= index + 4 + wanted {
                return String::from_utf8_lossy(&raw[index + 4..]).to_string();
            }
        }
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => return String::new(),
            Ok(read) => raw.extend_from_slice(&buffer[..read]),
        }
    }
}

/// The JSON-RPC answer for the tool named in `request`.
fn answer(table: &Arc<Mutex<Vec<(&'static str, &'static str)>>>, request: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(request).unwrap_or_default();
    let tool = parsed
        .get("params")
        .and_then(|params| params.get("name"))
        .and_then(|name| name.as_str())
        .unwrap_or("")
        .to_string();
    let table = table.lock().expect("table");
    let Some((_, verdict)) = table.iter().find(|(name, _)| *name == tool) else {
        return format!(
            r#"{{"jsonrpc":"2.0","id":1,"error":{{"code":-32602,"message":"tool '{tool}' not found: tool not found"}}}}"#
        );
    };
    let payload = format!(
        r#"{{"verdict":"{verdict}","environment":"FAT","selfIdc":"NTGXH","peerIdc":"NTGXH"}}"#
    );
    format!(
        r#"{{"jsonrpc":"2.0","id":1,"result":{{"content":[{{"type":"text","text":{}}}]}}}}"#,
        serde_json::to_string(&payload).unwrap()
    )
}
