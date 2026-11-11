// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! A loopback Captain stand-in driven by a route function.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One request the server saw.
#[derive(Clone, Debug)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub body: String,
    pub headers: Vec<(String, String)>,
}

impl Seen {
    /// The value of `name`, case-insensitively, or `""`.
    pub fn header(&self, name: &str) -> String {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    }
}

/// A mock Captain; `handler` decides the status and body of each request.
pub struct Server {
    pub base: String,
    seen: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    /// Serves `handler` on a random loopback port until dropped.
    pub fn start(handler: impl Fn(&Seen) -> (u16, String) + Send + Sync + 'static) -> Server {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback bind");
        listener.set_nonblocking(true).expect("non-blocking");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handler = Arc::new(handler);
        let handle = std::thread::spawn(move || loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    if let Some(request) = read_request(&mut stream) {
                        if let Ok(mut slot) = log.lock() {
                            slot.push(request.clone());
                        }
                        let (status, body) = handler(&request);
                        let response = format!(
                            "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(response.as_bytes());
                    }
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
        Server {
            base,
            seen,
            stop,
            handle: Some(handle),
        }
    }

    /// Everything the server was asked so far.
    pub fn requests(&self) -> Vec<Seen> {
        self.seen
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default()
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

/// Reads one request, headers and body included.
fn read_request(stream: &mut std::net::TcpStream) -> Option<Seen> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut raw = Vec::new();
    let mut buffer = [0u8; 4096];
    let head_end = loop {
        if let Some(index) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
            break index;
        }
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => return None,
            Ok(read) => raw.extend_from_slice(&buffer[..read]),
        }
    };
    let head = String::from_utf8_lossy(&raw[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next()?.split_whitespace();
    let method = request_line.next()?.to_string();
    let path = request_line.next()?.to_string();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect();
    let wanted: usize = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(0);
    let mut body = raw[head_end + 4..].to_vec();
    while body.len() < wanted {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => body.extend_from_slice(&buffer[..read]),
        }
    }
    Some(Seen {
        method,
        path,
        body: String::from_utf8_lossy(&body).to_string(),
        headers,
    })
}

/// Two SUCCESS releases; the newest one is first, as Captain orders them.
pub const RELEASES: &str = r#"{"result":[
  {"id":2,"status":"SUCCESS","diff":[{"key":"Image URL","to":"gitlab-ci/1:main-new"}]},
  {"id":1,"status":"SUCCESS","diff":[{"key":"Image URL","to":"gitlab-ci/1:main-old"}]}
]}"#;
