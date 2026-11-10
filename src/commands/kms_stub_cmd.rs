// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `kms-stub` command line: its own flags, banner and listener.
//!
//! Go gives this command a private `flag.FlagSet` with `ExitOnError`, so the
//! global flags never reach it and a bad flag exits 2 before anything is
//! bound. The module keeps that ordering: parse, validate, load, banner,
//! serve.

use std::io::Write;
use std::net::TcpListener;
use std::sync::Arc;

use crate::cli::{FlagSet, ParseError};

use super::kms_stub::{load, validate_listen, KmsStubData};
use super::kms_stub_http::{respond, Reply};

/// The default loopback address.
const DEFAULT_LISTEN: &str = "127.0.0.1:18080";

/// The default stub data file.
const DEFAULT_DATA: &str = "./kms-stub.yaml";

/// The `kms-stub` command.
///
/// Returns the process exit code; `2` is a refusal to start, not a runtime
/// failure.
pub fn kms_stub(args: &[String]) -> Result<i32, String> {
    let mut fs = FlagSet::new();
    fs.str_flag("listen", DEFAULT_LISTEN);
    fs.str_flag("data", DEFAULT_DATA);
    if let Err(e) = fs.parse(args) {
        // Go `flag` answers `-h` with the usage block and exit 0, and any
        // other bad flag with the error, the usage block and exit 2.
        if matches!(&e, ParseError::UnknownFlag { name } if name == "h" || name == "help") {
            print_usage();
            return Ok(0);
        }
        eprintln!("{e}");
        print_usage();
        return Ok(2);
    }
    let listen = fs
        .get("listen")
        .map(|v| v.as_str(DEFAULT_LISTEN))
        .unwrap_or_else(|| DEFAULT_LISTEN.to_string());
    let data_path = fs
        .get("data")
        .map(|v| v.as_str(DEFAULT_DATA))
        .unwrap_or_else(|| DEFAULT_DATA.to_string());

    if let Err(e) = validate_listen(&listen) {
        eprintln!("kms-stub: {e}");
        return Ok(2);
    }
    let data = match load(std::path::Path::new(&data_path)) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("kms-stub: {e}");
            return Ok(2);
        }
    };

    banner();
    log(&format!(
        "kms-stub listening on http://{listen} (data={data_path}, pwd={}, key={})",
        data.pwd.len(),
        data.key.len()
    ));
    log(&format!(
        "  build the runtime with -tags devkms and set KMS_ENDPOINT=http://{listen}"
    ));
    serve(&listen, &data)
}

/// Prints the three-line DEV ONLY banner on stdout.
fn banner() {
    println!("{}", "=".repeat(64));
    println!("  DEV ONLY — local KMS stub. Never point a deployed app at this.");
    println!("{}", "=".repeat(64));
}

/// Writes one `log.Printf` line to stderr, timestamped the way Go's `log`
/// package is by default.
fn log(message: &str) {
    let stamp = chrono::Local::now().format("%Y/%m/%d %H:%M:%S");
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{stamp} {message}");
}

/// Binds the address and serves until the process is stopped.
fn serve(listen: &str, data: &KmsStubData) -> Result<i32, String> {
    let listener = match TcpListener::bind(listen) {
        Ok(listener) => listener,
        Err(e) => {
            log(&format!("kms-stub: listen tcp {listen}: bind: {e}"));
            return Ok(1);
        }
    };
    let data = Arc::new(data.clone());
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        let data = Arc::clone(&data);
        let _ = std::thread::spawn(move || {
            while let Some(request) = crate::httpd::read_request(&mut stream) {
                let reply = respond(&data, &request);
                if send(&mut stream, &reply).is_err() {
                    return;
                }
            }
        });
    }
    Ok(0)
}

/// Writes one reply, adding the headers `net/http` always sends.
fn send(stream: &mut std::net::TcpStream, reply: &Reply) -> std::io::Result<()> {
    let date = crate::httpd::date_header();
    let mut headers: Vec<(&str, &str)> = vec![("Content-Type", reply.content_type)];
    if reply.nosniff {
        headers.push(("X-Content-Type-Options", "nosniff"));
    }
    headers.push(("Date", date.as_str()));
    crate::httpd::write_response(stream, reply.status, &headers, &reply.body)?;
    stream.flush()
}

/// Prints Go's `flag` usage block for this command.
fn print_usage() {
    eprintln!("Usage of kms-stub:");
    eprintln!("  -data string");
    eprintln!("    \tstub data file (git-ignored) (default \"./kms-stub.yaml\")");
    eprintln!("  -listen string");
    eprintln!("    \tloopback listen address (default \"127.0.0.1:18080\")");
}
