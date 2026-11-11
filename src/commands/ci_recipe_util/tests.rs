// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the `ci-recipe` download path.
//!
//! Every networked test stands up a loopback server, so the suite runs
//! without touching the real upstream instance.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::*;

/// `gzip -c` of `raw`, the fixture compressor this machine ships with.
fn gzip_bytes(raw: &[u8]) -> Vec<u8> {
    let mut child = Command::new("gzip")
        .arg("-c")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("gzip is installed");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(raw)
        .expect("gzip input written");
    let output = child.wait_with_output().expect("gzip finishes");
    assert!(output.status.success(), "gzip -c failed");
    output.stdout
}

/// A shell stub that answers `version` the way the real binary does.
fn stub_binary(version: &str) -> Vec<u8> {
    format!("#!/bin/sh\necho devine-utils-ci version {version}\n").into_bytes()
}

fn set_executable(path: &Path) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod +x");
}

/// Another parallel test's `fork` copies this test's fixture write
/// descriptor into its child for the microseconds before that child's
/// `exec` closes it (the descriptor is close-on-exec), so our own
/// `exec` of the same file can see `ETXTBSY`. The window is an
/// artefact of running tests concurrently, not of the behaviour under
/// test, so the calls that write a fixture and immediately execute it
/// retry until the other child has exec'd.
fn retry_busy<T>(mut operation: impl FnMut() -> Result<T, String>) -> Result<T, String> {
    let mut attempts = 0;
    loop {
        match operation() {
            Err(error) if error.to_lowercase().contains("text file busy") && attempts < 50 => {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(2));
            }
            result => return result,
        }
    }
}

/// A loopback server that answers every request with `status` and
/// `body`, until [`Server::shutdown`] stops it.
struct Server {
    /// The scheme://host:port prefix of the served URL.
    base_url: String,
    /// Tells the accept loop to exit.
    stop: Arc<AtomicBool>,
    /// The accept loop itself.
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
}

fn serve(status: &str, body: Vec<u8>) -> Server {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback bind");
    listener
        .set_nonblocking(true)
        .expect("non-blocking listener");
    let address = listener.local_addr().expect("local address");
    let status = status.to_string();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let handle = std::thread::spawn(move || loop {
        let accepted = match listener.accept() {
            Ok((stream, _)) => Some(stream),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(_) => break,
        };
        if let Some(mut stream) = accepted {
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => request.extend_from_slice(&buffer[..read]),
                }
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let head = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
            continue;
        }
        if thread_stop.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    });
    Server {
        base_url: format!("http://{address}"),
        stop,
        handle: Some(handle),
    }
}

fn recipe_for(base_url: &str, version: &str, install_dir: &Path) -> CIRecipe {
    CIRecipe {
        base_url: base_url.to_string(),
        version: version.to_string(),
        job_token: "test-token".to_string(),
        install_dir: install_dir.to_string_lossy().to_string(),
        ..CIRecipe::default()
    }
}

#[test]
fn downloads_installs_and_verifies_the_binary() {
    let server = serve("200 OK", gzip_bytes(&stub_binary("0.2.6")));
    let root = tempfile::tempdir().expect("temp dir");
    let install_dir = root.path().join("bin");
    let recipe = recipe_for(&server.base_url, "v0.2.6", &install_dir);

    let installed = retry_busy(|| download_ci_recipe_binary(&recipe)).expect("download succeeds");
    assert_eq!(
        installed,
        install_dir
            .join("devine-utils-ci")
            .to_string_lossy()
            .to_string()
    );
    let metadata = std::fs::metadata(&installed).expect("installed file");
    assert!(
        metadata.permissions().mode() & 0o100 != 0,
        "installed binary is not executable: {:?}",
        metadata.permissions()
    );
    let output = retry_busy(|| {
        Command::new(&installed)
            .arg("version")
            .output()
            .map_err(|error| error.to_string())
    })
    .expect("run installed");
    assert!(String::from_utf8_lossy(&output.stdout).contains("0.2.6"));
    server.shutdown();
}

#[test]
fn an_http_failure_creates_nothing() {
    let server = serve("404 Not Found", b"not found".to_vec());
    let root = tempfile::tempdir().expect("temp dir");
    let install_dir = root.path().join("bin");
    let recipe = recipe_for(&server.base_url, "v0.9.9", &install_dir);

    let error = download_ci_recipe_binary(&recipe).expect_err("404 fails");
    assert_eq!(error, "ci-recipe: HTTP 404 (404 Not Found): not found");
    assert!(
        !install_dir.exists(),
        "install dir created on the HTTP error path"
    );
    server.shutdown();
}

#[test]
fn an_html_error_page_is_not_gunzipped() {
    let server = serve("500 Internal Server Error", b"<html>boom</html>".to_vec());
    let root = tempfile::tempdir().expect("temp dir");
    let install_dir = root.path().join("bin");
    let recipe = recipe_for(&server.base_url, "v0.0.1", &install_dir);

    let error = download_ci_recipe_binary(&recipe).expect_err("500 fails");
    assert!(
        error.starts_with("ci-recipe: HTTP 500 (500 Internal Server Error): <html>boom</html>"),
        "{error}"
    );
    assert!(!install_dir.join("devine-utils-ci").exists());
    server.shutdown();
}

#[test]
fn a_version_mismatch_fails_after_the_install() {
    let server = serve("200 OK", gzip_bytes(&stub_binary("0.9.9")));
    let root = tempfile::tempdir().expect("temp dir");
    let install_dir = root.path().join("bin");
    let recipe = recipe_for(&server.base_url, "v0.2.6", &install_dir);

    let error = retry_busy(|| download_ci_recipe_binary(&recipe)).expect_err("mismatch");
    assert!(
        error.contains("version mismatch: want=0.2.6 got=0.9.9"),
        "{error}"
    );
    assert!(
        install_dir.join("devine-utils-ci").exists(),
        "the installed file survives a failed check"
    );
    server.shutdown();
}

#[test]
fn a_missing_job_token_is_refused_before_any_request() {
    let root = tempfile::tempdir().expect("temp dir");
    let mut recipe = recipe_for("http://127.0.0.1:1", "v0.2.6", root.path());
    recipe.job_token = String::new();

    let error = download_ci_recipe_binary(&recipe).expect_err("token required");
    assert_eq!(error, "ci-recipe: JobToken is empty (set $CI_JOB_TOKEN)");
}

#[test]
fn a_non_gzip_body_never_reaches_the_destination() {
    let server = serve("200 OK", b"<html>not gzip</html>".to_vec());
    let root = tempfile::tempdir().expect("temp dir");
    let install_dir = root.path().join("bin");
    let recipe = recipe_for(&server.base_url, "v0.2.6", &install_dir);

    let error = download_ci_recipe_binary(&recipe).expect_err("gzip required");
    assert_eq!(
        error,
        "ci-recipe: gunzip: not a valid gzip stream: gzip: invalid header"
    );
    assert!(install_dir.exists(), "the directory is made before gunzip");
    assert!(!install_dir.join("devine-utils-ci").exists());
    server.shutdown();
}

#[test]
fn a_truncated_stream_reports_unexpected_eof() {
    let mut body = gzip_bytes(&stub_binary("0.2.6"));
    body.truncate(body.len() - 4);
    let server = serve("200 OK", body);
    let root = tempfile::tempdir().expect("temp dir");
    let recipe = recipe_for(&server.base_url, "v0.2.6", &root.path().join("bin"));

    let error = download_ci_recipe_binary(&recipe).expect_err("truncated");
    assert_eq!(error, "ci-recipe: gunzip: copy: unexpected EOF");
    server.shutdown();
}

#[test]
fn the_gzip_probe_matches_new_reader() {
    assert_eq!(probe_gzip_header(b"").unwrap_err(), "EOF");
    assert_eq!(
        probe_gzip_header(&[0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0]).unwrap_err(),
        "unexpected EOF"
    );
    assert_eq!(
        probe_gzip_header(b"hello world!!").unwrap_err(),
        "gzip: invalid header"
    );
    assert_eq!(
        probe_gzip_header(&[0x1f, 0x8b, 9, 0, 0, 0, 0, 0, 0, 3]).unwrap_err(),
        "gzip: invalid header"
    );
    // Reserved flag bits are accepted, exactly like compress/gzip.
    probe_gzip_header(&[0x1f, 0x8b, 8, 0xe0, 0, 0, 0, 0, 0, 3]).expect("reserved flag bits pass");
    probe_gzip_header(&gzip_bytes(b"hi")).expect("a real stream passes");
}
mod io;
