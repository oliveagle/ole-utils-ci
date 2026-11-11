// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The wording Go gives a child process that did not exit cleanly.
//!
//! `os/exec` reports a non-zero exit as `exit status N` and a death by
//! signal as `signal: <name>` with the core-dump note
//! `ProcessState.String()` appends. Every command that quotes a failed
//! child — `doctor`'s probes and `release-check`'s git calls — spells it
//! exactly that way, so the wording lives in one place.

use std::process::ExitStatus;

/// Go's `ExitError.Error()` for `status`.
pub fn exit_status(status: &ExitStatus) -> String {
    if let Some(code) = status.code() {
        return format!("exit status {code}");
    }
    use std::os::unix::process::ExitStatusExt;
    let Some(signal) = status.signal() else {
        return "signal: killed".to_string();
    };
    let mut text = match signal_name(signal) {
        Some(name) => format!("signal: {name}"),
        None => format!("signal: signal {signal}"),
    };
    if status.core_dumped() {
        text.push_str(" (core dumped)");
    }
    text
}

/// Go's `syscall.Signal.String()` for the signals a child can die from;
/// `None` is the fallback Go spells `signal <n>`.
fn signal_name(signal: i32) -> Option<&'static str> {
    Some(match signal {
        1 => "hangup",
        2 => "interrupt",
        3 => "quit",
        4 => "illegal instruction",
        5 => "trace/breakpoint trap",
        6 => "aborted",
        7 => "bus error",
        8 => "floating point exception",
        9 => "killed",
        10 => "user defined signal 1",
        11 => "segmentation fault",
        12 => "user defined signal 2",
        13 => "broken pipe",
        14 => "alarm clock",
        15 => "terminated",
        16 => "stack fault",
        17 => "child exited",
        18 => "continued",
        19 => "stopped (signal)",
        20 => "stopped",
        21 => "stopped (tty input)",
        22 => "stopped (tty output)",
        23 => "urgent I/O condition",
        24 => "CPU time limit exceeded",
        25 => "file size limit exceeded",
        26 => "virtual timer expired",
        27 => "profiling timer expired",
        28 => "window changed",
        29 => "I/O possible",
        30 => "power failure",
        31 => "bad system call",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn exit_status_is_worded_the_way_go_words_it() {
        let exited = Command::new("sh")
            .arg("-c")
            .arg("exit 3")
            .output()
            .expect("sh");
        assert_eq!(exit_status(&exited.status), "exit status 3");

        let killed = Command::new("sh")
            .arg("-c")
            .arg("kill -9 $$")
            .output()
            .expect("sh");
        assert_eq!(exit_status(&killed.status), "signal: killed");
    }
}
