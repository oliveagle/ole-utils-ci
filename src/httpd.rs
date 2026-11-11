// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! A very small HTTP/1.1 server for the local stub commands.
//!
//! The Go original uses `net/http`. Pulling a HTTP crate in would cost a
//! dependency the gates cannot audit (and `blaze` cannot always compile), and
//! the two stubs need exactly one thing: read a request line, its headers and
//! its body, then write a response with a `Content-Length`. That is what this
//! module does, on a plain [`TcpStream`].

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// How long a connection may sit idle between two keep-alive requests.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(5);

/// One parsed request.
#[derive(Debug, Default)]
pub struct Request {
    /// The HTTP method, upper-case.
    pub method: String,
    /// The request target, without the query string.
    pub path: String,
    /// Header names lower-cased, in the order they arrived.
    pub headers: Vec<(String, String)>,
    /// The body, already decoded with `Content-Length`.
    pub body: Vec<u8>,
}

impl Request {
    /// The value of one header, case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Reads one request, or `None` when the peer closed the connection.
///
/// A timeout or a malformed request is reported as `None` too: both mean the
/// exchange is over and the listener should move on to the next connection.
pub fn read_request(stream: &mut TcpStream) -> Option<Request> {
    stream.set_read_timeout(Some(IDLE_TIMEOUT)).ok()?;
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        if let Some(index) = find_headers_end(&buffer) {
            break index;
        }
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > 1024 * 1024 {
            return None;
        }
    };

    let head = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let request_line = lines.next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let path = target.split('?').next().unwrap_or(&target).to_string();

    let mut headers = Vec::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_lowercase(), value.trim().to_string()));
        }
    }

    let mut body = buffer[header_end + 4..].to_vec();
    let wanted: usize = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(0);
    while body.len() < wanted {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(wanted);

    Some(Request {
        method,
        path,
        headers,
        body,
    })
}

/// The byte offset of the `\r\n\r\n` that ends the header block.
fn find_headers_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

/// Writes a complete response and flushes it.
pub fn write_response(
    stream: &mut TcpStream,
    status: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> std::io::Result<()> {
    let mut out = format!("HTTP/1.1 {status}\r\n");
    for (name, value) in headers {
        out.push_str(name);
        out.push_str(": ");
        out.push_str(value);
        out.push_str("\r\n");
    }
    out.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
    stream.write_all(out.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

/// The `Date` header, formatted the way `net/http` formats it.
pub fn date_header() -> String {
    chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S GMT")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_block_is_located() {
        assert_eq!(find_headers_end(b"GET / HTTP/1.1\r\n\r\n"), Some(14));
        assert_eq!(find_headers_end(b"GET / HTTP/1.1\r\n"), None);
    }

    #[test]
    fn a_date_header_looks_like_rfc1123() {
        let value = date_header();
        assert!(value.ends_with(" GMT"));
        assert!(value.contains(','));
        assert_eq!(value.len(), 29);
    }
}
mod outbound;

pub use outbound::{download, request, Response};
