// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The command itself kills processes, so the tests only exercise the pieces
//! that make that safe: the port probe refuses an idle port, and the signal
//! path reports a missing pid instead of pretending it worked. Nothing here
//! touches the PID directory or the helper processes.

use super::*;

#[test]
fn an_idle_port_reports_that_nothing_is_listening() {
    // Port 1 is reserved and no helper ever binds it.
    let error = kill_on_port(1).expect_err("refused");
    assert_eq!(error, "no process on port 1");
}

#[test]
fn an_unknown_process_name_reports_nothing_found() {
    let error = kill_by_name("devine-utils-ci-no-such-helper").expect_err("refused");
    assert!(error.contains("no devine-utils-ci-no-such-helper process found"));
}

#[test]
fn killing_a_missing_pid_is_an_error_not_a_silent_success() {
    let error = signal_kill(999_999_999).expect_err("refused");
    assert!(error.starts_with("kill 999999999"));
}

#[test]
fn the_known_port_table_names_the_helper() {
    assert_eq!(KNOWN_PORTS.len(), 1);
    assert_eq!(KNOWN_PORTS[0], (18080, "fake-metadata-server"));
}

#[test]
fn the_helper_names_are_the_ones_go_sweeps_for() {
    assert_eq!(HELPER_NAMES, ["fake-metadata-server"]);
}
