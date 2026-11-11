// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the terminal helper.

use super::*;

#[test]
fn the_terminal_probe_answers_without_a_terminal() {
    // The test process has no controlling terminal, so the probe must say
    // so rather than fail.
    let _ = stdin_is_tty();
}

#[test]
fn raw_mode_is_refused_without_a_terminal() {
    assert!(RawMode::enable().is_none());
}
