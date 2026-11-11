// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for `pipeline-run`.
//!
//! Every networked test stands up a loopback server, so the suite runs
//! without touching the real GitLab instance.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::model::{parse_variables, render_output, truncate, PipelineRunResult};
use super::*;

/// `&["a", "b"]` -> `["a".to_string(), "b".to_string()]`.
fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

/// One request the fake GitLab saw.
#[derive(Clone)]
struct Seen {
    method: String,
    path: String,
    body: String,
}

/// A loopback GitLab stand-in that records what it was asked.
struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    /// Stops the accept loop and waits for it to finish.
    fn shutdown(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Everything the server was asked so far.
    fn requests(&self) -> Vec<Seen> {
        self.seen.lock().map(|log| log.clone()).unwrap_or_default()
    }
}

/// Serves `handler` on a random loopback port until [`Server::shutdown`].
fn serve<F>(handler: F) -> Server
where
    F: Fn(&Seen) -> (String, Vec<u8>) + Send + Sync + 'static,
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
                let (status, body) = handler(&request);
                let header = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
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

/// Reads one request, headers and `Content-Length` body included.
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
    let wanted: usize = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(0);
    let mut body = raw[head_end + 4..].to_vec();
    while body.len() < wanted {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => body.extend_from_slice(&buffer[..read]),
        }
    }
    body.truncate(wanted);
    Some(Seen {
        method,
        path,
        body: String::from_utf8_lossy(&body).to_string(),
    })
}

/// Answers the trigger with `id`, then reports `status` for every poll.
fn gitlab(
    id: i64,
    status: &'static str,
) -> impl Fn(&Seen) -> (String, Vec<u8>) + Send + Sync + 'static {
    move |request| {
        if request.method == "POST" && request.path.ends_with("/pipeline") {
            let body = format!(
                "{{\"id\":{id},\"status\":\"created\",\"ref\":\"main\",\
                 \"web_url\":\"http://x/{id}\"}}"
            );
            return ("201 Created".to_string(), body.into_bytes());
        }
        if request.method == "GET" && request.path.contains(&format!("/pipelines/{id}")) {
            let body = format!("{{\"id\":{id},\"status\":\"{status}\"}}");
            return ("200 OK".to_string(), body.into_bytes());
        }
        ("404 Not Found".to_string(), b"nope".to_vec())
    }
}

#[test]
fn usage_block_is_go_byte_for_byte() {
    assert_eq!(USAGE.len(), 1859);
    assert!(USAGE.starts_with("devine-utils-ci pipeline-run — trigger a GitLab pipeline.\n"));
    assert!(USAGE.ends_with("    1  fatal error (auth, network, bad args)\n"));
}

#[test]
fn missing_project_path_prints_usage_and_errors() {
    let error = run(&args(&[]), Timeouts::default()).expect_err("must refuse");
    assert_eq!(error, MISSING_PROJECT);
}

#[test]
fn flags_after_the_positional_are_still_read() {
    let (flags, positional) = parse_flags(&args(&[
        "faas/x",
        "--ref",
        "dev",
        "--var",
        "A=1",
        "--var=B=2",
        "--wait",
        "--wait-timeout",
        "5",
    ]))
    .expect("parses");
    assert_eq!(positional, vec!["faas/x".to_string()]);
    assert_eq!(flags.reference, "dev");
    assert_eq!(flags.variables, vec!["A=1", "B=2"]);
    assert!(flags.wait);
    assert_eq!(flags.wait_timeout, 5);
}

#[test]
fn the_two_output_flags_share_one_go_variable() {
    let (flags, _) = parse_flags(&args(&["p", "--output", "json", "-o", "text"])).expect("parses");
    assert_eq!(flags.output, "text");
    let (flags, _) = parse_flags(&args(&["p", "-o", "text", "--output=json"])).expect("parses");
    assert_eq!(flags.output, "json");
}

#[test]
fn an_unknown_flag_is_reported_the_way_go_words_it() {
    let error = run(&args(&["--bogus"]), Timeouts::default()).expect_err("must refuse");
    assert_eq!(error, "flag provided but not defined: -bogus");
}

#[test]
fn variables_split_on_the_first_equals() {
    let map = parse_variables(&args(&["A=1=2", "B", "A=3"]));
    assert_eq!(map.get("A").map(String::as_str), Some("3"));
    assert_eq!(map.get("B").map(String::as_str), Some(""));
}

#[test]
fn result_json_keeps_go_field_order_and_omissions() {
    let variables = parse_variables(&args(&["B=2", "A=1"]));
    let mut result =
        PipelineRunResult::new("faas/x", "main", variables, "http://git", "--token flag");
    let json = render_output("text", false, &result).expect("encodes");
    assert!(
        json.starts_with("{\"project\":\"faas/x\",\"ref\":\"main\",\"status\":\"\""),
        "{json}"
    );
    assert!(
        json.contains("\"created_at\":\"0001-01-01T00:00:00Z\""),
        "{json}"
    );
    assert!(!json.contains("\"message\""), "{json}");
    assert!(!json.contains("\"pipeline_id\""), "{json}");
    assert!(!json.contains("\"waited_seconds\""), "{json}");
    assert!(
        json.contains("\"variables\":{\"A\":\"1\",\"B\":\"2\"}"),
        "{json}"
    );
    result.status = "created".to_string();
    result.pipeline_id = 7;
    result.message = "Pipeline #7 created on main".to_string();
    let json = render_output("text", false, &result).expect("encodes");
    assert!(json.contains("\"pipeline_id\":7"), "{json}");
    assert!(json.contains("\"message\":"), "{json}");
}

#[test]
fn truncate_backs_off_a_char_boundary() {
    assert_eq!(truncate("short", 10), "short");
    assert_eq!(truncate("abcdef", 3), "abc...(truncated)");
    assert_eq!(truncate("日本語です", 4), "日...(truncated)");
}
mod network;
