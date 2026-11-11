// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the key decoder.

use super::*;

#[test]
fn decodes_the_plain_keys() {
    assert_eq!(decode(b"q"), vec![Key::Quit]);
    assert_eq!(decode(b"Q"), vec![Key::Quit]);
    assert_eq!(decode(b"\x03"), vec![Key::Quit]);
    assert_eq!(decode(b"r"), vec![Key::Refresh]);
    assert_eq!(decode(b"j"), vec![Key::Down]);
    assert_eq!(decode(b"k"), vec![Key::Up]);
    assert_eq!(decode(b"+"), vec![Key::IntervalUp]);
    assert_eq!(decode(b"="), vec![Key::IntervalUp]);
    assert_eq!(decode(b"-"), vec![Key::IntervalDown]);
    assert_eq!(decode(b"o"), vec![Key::Open]);
    assert_eq!(decode(b"z"), vec![Key::None]);
}

#[test]
fn decodes_the_arrow_keys() {
    assert_eq!(decode(b"\x1b[A"), vec![Key::Up]);
    assert_eq!(decode(b"\x1b[B"), vec![Key::Down]);
}

#[test]
fn decodes_the_page_keys_as_arrows() {
    assert_eq!(decode(b"\x1b[5~"), vec![Key::Up]);
    assert_eq!(decode(b"\x1b[6~"), vec![Key::Down]);
}

#[test]
fn a_lone_escape_quits() {
    assert_eq!(decode(b"\x1b"), vec![Key::Quit]);
    assert_eq!(decode(b"\x1b[Z"), vec![Key::Quit]);
}

#[test]
fn a_quit_key_stops_the_rest_of_the_buffer() {
    assert_eq!(decode(b"qj"), vec![Key::Quit]);
}

#[test]
fn the_line_reader_maps_the_scripted_keys() {
    assert_eq!(decode_line("q"), Key::Quit);
    assert_eq!(decode_line("Q"), Key::Quit);
    assert_eq!(decode_line("r"), Key::Refresh);
    assert_eq!(decode_line("j"), Key::Down);
    assert_eq!(decode_line("k"), Key::Up);
    assert_eq!(decode_line("+"), Key::IntervalUp);
    assert_eq!(decode_line("-"), Key::IntervalDown);
    assert_eq!(decode_line("o"), Key::Open);
    assert_eq!(decode_line("anything"), Key::None);
}
