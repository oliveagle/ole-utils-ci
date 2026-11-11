// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! A loopback GitLab stand-in for the `pipeline-watch` tests.
//!
//! The dashboard talks to a real GitLab over HTTP, so the tests stand up
//! a socket on the loopback interface and answer the handful of endpoints
//! the dashboard reads. Every request is recorded, so a test can assert
//! on the URL shapes as well as on the rendered rows.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One request the fake GitLab saw.
#[derive(Clone, Debug)]
pub struct Seen {
    /// The request line's method.
    pub method: String,
    /// The request line's path, with the query string.
    pub path: String,
}

/// The reply the fake GitLab sends.
pub struct Reply {
    /// The HTTP status code.
    pub status: u16,
    /// The response body.
    pub body: String,
    /// Extra response headers.
    pub headers: Vec<(String, String)>,
}

impl Reply {
    /// A `200` reply with one JSON body.
    pub fn json(body: &str) -> Reply {
        Reply {
            status: 200,
            body: body.to_string(),
            headers: Vec::new(),
        }
    }

    /// A reply with extra headers.
    pub fn with_headers(mut self, headers: &[(&str, &str)]) -> Reply {
        for (name, value) in headers {
            self.headers.push((name.to_string(), value.to_string()));
        }
        self
    }

    /// A `404` reply.
    pub fn missing() -> Reply {
        Reply {
            status: 404,
            body: "404 Not Found".to_string(),
            headers: Vec::new(),
        }
    }
}

/// A loopback GitLab stand-in.
pub struct Server {
    /// The base URL the client should talk to.
    pub base_url: String,
    seen: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    /// Stops the accept loop and waits for it to finish.
    pub fn shutdown(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Every request the server was asked so far.
    pub fn requests(&self) -> Vec<Seen> {
        self.seen.lock().map(|log| log.clone()).unwrap_or_default()
    }

    /// The paths of every request, in order.
    pub fn paths(&self) -> Vec<String> {
        self.requests()
            .into_iter()
            .map(|request| request.path)
            .collect()
    }
}

/// Serves `handler` on a random loopback port until [`Server::shutdown`].
pub fn serve<F>(handler: F) -> Server
where
    F: Fn(&Seen) -> Reply + Send + Sync + 'static,
{
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback bind");
    listener
        .set_nonblocking(true)
        .expect("non-blocking listener");
    let address = listener.local_addr().expect("local address");
    let base_url = format!("http://{address}");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let thread_seen = Arc::clone(&seen);
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let handle = std::thread::spawn(move || loop {
        let accepted = match listener.accept() {
            Ok((stream, _)) => Some(stream),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(_) => break,
        };
        if let Some(mut stream) = accepted {
            if let Some(request) = read_request(&mut stream) {
                if let Ok(mut log) = thread_seen.lock() {
                    log.push(request.clone());
                }
                let reply = handler(&request);
                let mut head = format!(
                    "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nConnection: close\r\n",
                    reply.status,
                    reply.body.len()
                );
                for (name, value) in &reply.headers {
                    head.push_str(&format!("{name}: {value}\r\n"));
                }
                head.push_str("\r\n");
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(reply.body.as_bytes());
            }
            continue;
        }
        if thread_stop.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    });
    Server {
        base_url,
        seen,
        stop,
        handle: Some(handle),
    }
}

/// Reads one request, headers included.
fn read_request(stream: &mut TcpStream) -> Option<Seen> {
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
    let request_line = lines.next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    Some(Seen { method, path })
}
