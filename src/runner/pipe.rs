// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `producer | consumer > out_path`, assembled without a shell.
//!
//! The Go original builds the pipe by hand so the same code works on Windows,
//! and so `sh -c` never enters the picture: the producer's standard output
//! *and* standard error both feed the consumer's stdin (the template's
//! `2>&1`), the consumer's standard output lands in `out_path`, and its
//! standard error stays on the terminal.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use super::{exit_text, Runner};

impl Runner {
    /// Runs `producer | consumer > out_path` without a shell.
    ///
    /// A producer failure is forgiven when the consumer succeeded and wrote
    /// something — the normal early-close case of a pipe.
    pub fn run_pipe(
        &self,
        producer: &[&str],
        consumer: &[&str],
        out_path: &Path,
    ) -> Result<(), String> {
        if producer.is_empty() || consumer.is_empty() {
            return Err("run_pipe: empty command".to_string());
        }
        if self.verbose {
            println!(
                "[pipe] {} | {} > {}",
                producer.join(" "),
                consumer.join(" "),
                out_path.display()
            );
        }
        let mut sink_child = self.spawn_consumer(consumer, out_path)?;
        let mut source_child = match self.spawn_producer(producer) {
            Ok(child) => child,
            Err(e) => {
                let _ = sink_child.kill();
                let _ = sink_child.wait();
                return Err(e);
            }
        };
        pump(&mut sink_child, &mut source_child, producer[0], consumer[0])?;
        let source_status = wait_child(source_child, producer[0])?;
        let sink_status = wait_child(sink_child, consumer[0])?;
        verdict(
            producer[0],
            &source_status,
            consumer[0],
            &sink_status,
            out_path,
        )
    }

    /// Starts the consumer with its stdin piped and its stdout in `out_path`.
    fn spawn_consumer(&self, consumer: &[&str], out_path: &Path) -> Result<Child, String> {
        let file = std::fs::File::create(out_path)
            .map_err(|e| format!("create {}: {e}", out_path.display()))?;
        Command::new(consumer[0])
            .args(&consumer[1..])
            .current_dir(&self.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::from(file))
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("start {}: {e}", consumer[0]))
    }

    /// Starts the producer with both of its output streams captured.
    fn spawn_producer(&self, producer: &[&str]) -> Result<Child, String> {
        Command::new(producer[0])
            .args(&producer[1..])
            .current_dir(&self.dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("start {}: {e}", producer[0]))
    }
}

/// Moves the producer's output into the consumer's stdin until both end.
fn pump(
    sink_child: &mut Child,
    source_child: &mut Child,
    producer: &str,
    consumer: &str,
) -> Result<(), String> {
    let stdin = sink_child
        .stdin
        .take()
        .ok_or_else(|| format!("start {consumer}: stdin unavailable"))?;
    let sink = Arc::new(Mutex::new(stdin));
    let sources = take_sources(source_child, producer)?;
    let mut handles = Vec::new();
    for source in sources {
        let sink = Arc::clone(&sink);
        handles.push(std::thread::spawn(move || drain(source, sink)));
    }
    // The threads hold the only remaining handles, so the consumer sees EOF
    // when the last one finishes.
    drop(sink);
    for handle in handles {
        let _ = handle.join();
    }
    Ok(())
}

/// Takes the producer's stdout and stderr for the pump threads.
fn take_sources(child: &mut Child, producer: &str) -> Result<Vec<Box<dyn Read + Send>>, String> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("start {producer}: stdout unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("start {producer}: stderr unavailable"))?;
    Ok(vec![Box::new(stdout), Box::new(stderr)])
}

/// Copies one producer stream into the consumer's stdin until EOF.
fn drain(mut source: Box<dyn Read + Send>, sink: Arc<Mutex<std::process::ChildStdin>>) {
    let mut buffer = [0u8; 8192];
    loop {
        match source.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => match sink.lock() {
                Ok(mut stdin) => {
                    if stdin.write_all(&buffer[..count]).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            },
            Err(_) => break,
        }
    }
}

/// Waits for one side of the pipe.
fn wait_child(mut child: Child, program: &str) -> Result<std::process::ExitStatus, String> {
    child.wait().map_err(|e| format!("wait {program}: {e}"))
}

/// The pipeline's verdict, forgiving a producer that lost the race to a
/// consumer which still wrote something.
fn verdict(
    producer: &str,
    producer_status: &std::process::ExitStatus,
    consumer: &str,
    consumer_status: &std::process::ExitStatus,
    out_path: &Path,
) -> Result<(), String> {
    if !consumer_status.success() {
        return Err(format!("{consumer}: {}", exit_text(consumer_status)));
    }
    if producer_status.success() {
        return Ok(());
    }
    let written = std::fs::metadata(out_path).map(|m| m.len()).unwrap_or(0);
    if written > 0 {
        return Ok(());
    }
    Err(format!("{producer}: {}", exit_text(producer_status)))
}
