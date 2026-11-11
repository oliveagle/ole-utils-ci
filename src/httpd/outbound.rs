// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The outbound side of the HTTP helper: one exchange, by way of `curl`.
//!
//! The Go original uses `net/http`; a Rust HTTP stack would cost a TLS
//! dependency the gates cannot audit, so every outbound call shells out to
//! `curl` — the same binary the `ci-recipe` setup script already requires —
//! and parses its `-i` output here.

use std::io::Write;
use std::time::Duration;

use super::find_headers_end;

/// One response from [`request`] or [`download`].
#[derive(Debug, Clone)]
pub struct Response {
    /// Status code of the final exchange.
    pub status: u16,
    /// The reason phrase (`Not Found`), exactly as the status line carried it.
    pub text: String,
    /// Response headers of the final exchange, lower-cased.
    pub headers: Vec<(String, String)>,
    /// The body, already de-chunked by curl.
    pub body: Vec<u8>,
}

impl Response {
    /// `404 Not Found` — the spelling `net/http` puts in `resp.Status`.
    pub fn status_display(&self) -> String {
        if self.text.is_empty() {
            self.status.to_string()
        } else {
            format!("{} {}", self.status, self.text)
        }
    }

    /// The body as (lossy) UTF-8.
    pub fn body_string(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }

    /// One response header, looked up case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Performs one HTTP exchange (following redirects the way `net/http` does).
///
/// The Go original uses `net/http`; a Rust HTTP stack would cost a TLS
/// dependency the gates cannot audit, so every outbound call shells out to
/// `curl` — the same binary the `ci-recipe` setup script already requires —
/// and parses its `-i` output here.
pub fn request(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
    timeout: Duration,
) -> Result<Response, String> {
    let mut current_method = method.to_string();
    let mut current_url = url.to_string();
    let mut current_body = body.map(<[u8]>::to_vec);
    for _ in 0..10 {
        let response = exchange(
            &current_method,
            &current_url,
            headers,
            current_body.as_deref(),
            timeout,
        )?;
        let redirect = matches!(response.status, 301 | 302 | 303 | 307 | 308);
        let location = response.header("location").map(str::to_string);
        if !redirect || location.is_none() {
            return Ok(response);
        }
        // `net/http`'s default policy: 303 becomes GET, 301/302 turn POST
        // into GET, and 307/308 keep method and body.
        let downgrade = (response.status == 303 && current_method != "HEAD")
            || (matches!(response.status, 301 | 302) && current_method == "POST");
        if downgrade {
            current_method = "GET".to_string();
            current_body = None;
        }
        current_url = resolve_location(&current_url, location.as_deref().unwrap_or(""));
    }
    Err(format!("stopped after 10 redirects following {url}"))
}

/// Downloads `url` to `path` with `curl -L`, returning the final status code.
///
/// The body goes straight to the file, so a release binary never sits in
/// memory; the status arrives on stdout through `-w`.
pub fn download(
    url: &str,
    path: &std::path::Path,
    headers: &[(&str, &str)],
    timeout: Duration,
) -> Result<u16, String> {
    let mut command = curl_command(timeout);
    command.arg("-L");
    for (name, value) in headers {
        command.arg("-H").arg(format!("{name}: {value}"));
    }
    command.arg("-o").arg(path);
    command.arg("-w").arg("%{http_code}");
    command.arg(url);
    let output = command
        .output()
        .map_err(|e| format!("failed to run curl: {e}"))?;
    let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() || status.is_empty() || status == "000" {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        if detail.is_empty() {
            return Err(format!("download {url} failed"));
        }
        return Err(format!("download {url}: {detail}"));
    }
    status
        .parse::<u16>()
        .map_err(|_| format!("download {url}: unexpected status {status:?}"))
}

/// The `curl` skeleton every outbound call starts from: silent but loud on
/// errors, HTTP/1.1 so the status line parses the way `net/http` reads it,
/// and a wall-clock budget.
fn curl_command(timeout: Duration) -> std::process::Command {
    let mut command = std::process::Command::new("curl");
    command
        .arg("-sS")
        .arg("--http1.1")
        .arg("--max-time")
        .arg(timeout.as_secs().max(1).to_string());
    command
}

/// One `curl -i` exchange, parsed into a [`Response`].
fn exchange(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
    timeout: Duration,
) -> Result<Response, String> {
    let mut command = curl_command(timeout);
    command.arg("-X").arg(method).arg("-i");
    for (name, value) in headers {
        command.arg("-H").arg(format!("{name}: {value}"));
    }
    if body.is_some() {
        command.arg("--data-binary").arg("@-");
    }
    command.arg("--").arg(url);
    command
        .stdin(if body.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to run curl: {e}"))?;
    if let (Some(mut stdin), Some(body)) = (child.stdin.take(), body) {
        stdin
            .write_all(body)
            .map_err(|e| format!("curl {url}: {e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("curl {url}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        if detail.is_empty() {
            return Err(format!("{method} {url} failed"));
        }
        return Err(format!("{method} {url}: {detail}"));
    }
    parse_response(&output.stdout, method, url)
}

/// Splits curl's `-i` output into status line, headers and body.
fn parse_response(raw: &[u8], method: &str, url: &str) -> Result<Response, String> {
    let head_end =
        find_headers_end(raw).ok_or_else(|| format!("{method} {url}: no response headers"))?;
    let head = String::from_utf8_lossy(&raw[..head_end]);
    let mut lines = head.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| format!("{method} {url}: empty response"))?;
    let after_version = status_line
        .split_once(' ')
        .map(|(_, rest)| rest)
        .unwrap_or(status_line);
    let (code, text) = match after_version.split_once(' ') {
        Some((code, text)) => (code, text),
        None => (after_version, ""),
    };
    let status: u16 = code
        .parse()
        .map_err(|_| format!("{method} {url}: bad status line {status_line:?}"))?;
    let mut headers = Vec::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_lowercase(), value.trim().to_string()));
        }
    }
    Ok(Response {
        status,
        text: text.to_string(),
        headers,
        body: raw[head_end + 4..].to_vec(),
    })
}

/// Resolves a `Location` header against the URL that produced it.
fn resolve_location(base: &str, location: &str) -> String {
    if location.contains("://") {
        return location.to_string();
    }
    let scheme_end = base.find("://").map(|index| index + 3).unwrap_or(0);
    let authority_end = base[scheme_end..]
        .find('/')
        .map(|index| scheme_end + index)
        .unwrap_or(base.len());
    let origin = &base[..authority_end];
    if let Some(rest) = location.strip_prefix('/') {
        return format!("{origin}/{rest}");
    }
    let path_start = authority_end;
    let path = &base[path_start..];
    let dir_end = path.rfind('/').map(|index| index + 1).unwrap_or(0);
    format!("{origin}{}{location}", &path[..dir_end])
}

#[cfg(test)]
mod outbound_tests {
    use super::*;

    #[test]
    fn parses_a_curl_response_block() {
        let raw = b"HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\n\r\nnope";
        let response = parse_response(raw, "GET", "http://x").unwrap();
        assert_eq!(response.status, 404);
        assert_eq!(response.status_display(), "404 Not Found");
        assert_eq!(response.header("Content-Type"), Some("text/plain"));
        assert_eq!(response.body_string(), "nope");
    }

    #[test]
    fn resolves_absolute_and_relative_locations() {
        assert_eq!(
            resolve_location("http://a/b/c", "https://other/x"),
            "https://other/x"
        );
        assert_eq!(resolve_location("http://a/b/c", "/z"), "http://a/z");
        assert_eq!(resolve_location("http://a/b/c", "z"), "http://a/b/z");
        assert_eq!(resolve_location("http://a/b/c?y=1", "z"), "http://a/b/z");
    }
}
