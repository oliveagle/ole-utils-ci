// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `fake-metadata-server` command.
//!
//! Ctrip's foundation package resolves its configuration through a metadata
//! endpoint in `init()`, so a CI box without corporate network access panics
//! before the first test runs. This stub answers that one endpoint, which is
//! all the runtime needs to start.
//!
//! By default the command daemonises itself by re-invoking the current
//! executable with `--run`; pass `--run` (or `-f`) to stay in the foreground.

use std::io::Write;
use std::net::TcpStream;
use std::process::{Command, Stdio};

use crate::httpd::{date_header, read_request, write_response};

/// The single path the stub serves.
const ENDPOINT: &str = "/api/ctrip/metadata.json";

/// The exact body `net/http` produces: Go marshals map keys in sorted order
/// and `json.Encoder` terminates the value with a newline.
const BODY: &str = "{\"data\":{},\"message\":\"ok\",\"status\":0}\n";

/// The default listen port.
const DEFAULT_PORT: &str = "18080";

/// The `fake-metadata-server` command.
pub fn fake_metadata(args: &[String]) -> Result<i32, String> {
    let foreground = args.iter().any(|arg| arg == "--run" || arg == "-f");
    let rest: Vec<String> = args
        .iter()
        .filter(|arg| *arg != "--run" && *arg != "-f")
        .cloned()
        .collect();
    if !foreground {
        let exe =
            std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("devine-utils-ci"));
        let child = Command::new(exe)
            .arg("fake-metadata-server")
            .arg("--run")
            .args(&rest)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("fake-metadata-server start failed: {e}"))?;
        println!("fake-metadata-server started (pid {})", child.id());
        return Ok(0);
    }
    serve()
}

/// The listen port, from `FAKE_METADATA_PORT`.
fn port() -> String {
    match std::env::var("FAKE_METADATA_PORT") {
        Ok(value) if !value.is_empty() => value,
        _ => DEFAULT_PORT.to_string(),
    }
}

/// Binds the endpoint and serves until the process is stopped.
fn serve() -> Result<i32, String> {
    let port = port();
    let address = format!("0.0.0.0:{port}");
    let listener =
        std::net::TcpListener::bind(&address).map_err(|e| format!("fake-metadata-server: {e}"))?;
    let stamp = chrono::Local::now().format("%Y/%m/%d %H:%M:%S");
    eprintln!("{stamp} fake-metadata-server listening on :{port}");
    eprintln!("{stamp}   set METADATA_DOMAIN=http://localhost:{port} before running tests");
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        std::thread::spawn(move || serve_connection(&mut stream));
    }
    Ok(0)
}

/// Serves one keep-alive connection until the peer goes away.
fn serve_connection(stream: &mut TcpStream) {
    while let Some(request) = read_request(stream) {
        let date = date_header();
        let (status, headers, body): (&str, Vec<(&str, &str)>, &[u8]) = if request.path == ENDPOINT
        {
            (
                "200 OK",
                vec![
                    ("Content-Type", "application/json"),
                    ("Date", date.as_str()),
                ],
                BODY.as_bytes(),
            )
        } else {
            (
                "404 Not Found",
                vec![
                    ("Content-Type", "text/plain; charset=utf-8"),
                    ("X-Content-Type-Options", "nosniff"),
                    ("Date", date.as_str()),
                ],
                b"404 page not found\n",
            )
        };
        if write_response(stream, status, &headers, body).is_err() {
            return;
        }
        let _ = stream.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_is_the_go_encoding() {
        assert_eq!(BODY, "{\"data\":{},\"message\":\"ok\",\"status\":0}\n");
        assert_eq!(BODY.len(), 38);
    }

    #[test]
    fn the_endpoint_is_the_ctrip_metadata_path() {
        assert_eq!(ENDPOINT, "/api/ctrip/metadata.json");
    }

    #[test]
    fn the_port_falls_back_to_the_default() {
        // The env var is process-wide, so only assert the constant.
        assert_eq!(DEFAULT_PORT, "18080");
    }
}
