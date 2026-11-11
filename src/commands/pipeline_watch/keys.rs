// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The key events the dashboard accepts.
//!
//! Two readers exist. On a terminal the dashboard switches stdin to raw
//! mode, so every keystroke arrives without a newline and the arrow keys
//! arrive as the usual escape sequences. Anywhere else — a pipe, a CI
//! runner, a here-doc — the reader takes one line per keystroke, which is
//! what the scripted usage has always used.

use std::io::{BufRead, Read};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// One key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Nothing the dashboard acts on.
    None,
    /// Leave the dashboard.
    Quit,
    /// Force a refresh.
    Refresh,
    /// Move the selection up.
    Up,
    /// Move the selection down.
    Down,
    /// Lengthen the refresh interval.
    IntervalUp,
    /// Shorten the refresh interval.
    IntervalDown,
    /// Open the selected repo in the browser.
    Open,
}

/// Starts the key reader and returns its channel.
///
/// The reader ends with a quit event when stdin closes.
pub fn spawn_reader(raw: bool) -> Receiver<Key> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        if raw {
            read_raw(&sender);
        } else {
            read_cooked(&sender);
        }
    });
    receiver
}

/// Reads raw bytes and decodes them into key events.
fn read_raw(sender: &Sender<Key>) {
    let mut stdin = std::io::stdin();
    let mut buffer = [0u8; 64];
    loop {
        let count = match stdin.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(_) => break,
        };
        for key in decode(&buffer[..count]) {
            let _ = sender.send(key);
            if key == Key::Quit {
                return;
            }
        }
    }
    let _ = sender.send(Key::Quit);
}

/// Reads one line per keystroke.
fn read_cooked(sender: &Sender<Key>) {
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    let mut line = String::new();
    loop {
        line.clear();
        match handle.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        let key = decode_line(line.trim_end_matches(['\r', '\n']));
        let _ = sender.send(key);
        if key == Key::Quit {
            return;
        }
    }
    let _ = sender.send(Key::Quit);
}

/// Decodes a buffer of raw bytes into key events.
///
/// The escape sequences the dashboard understands:
///
/// ```text
/// ESC          quit
/// ESC [ A      up
/// ESC [ B      down
/// ESC [ 5 ~    page up, treated as up
/// ESC [ 6 ~    page down, treated as down
/// ```
pub fn decode(input: &[u8]) -> Vec<Key> {
    let mut out: Vec<Key> = Vec::new();
    let mut index = 0;
    while index < input.len() {
        let byte = input[index];
        if byte == 0x1b {
            match escape_key(input, index) {
                Some((key, next)) => {
                    out.push(key);
                    index = next;
                    continue;
                }
                None => {
                    out.push(Key::Quit);
                    return out;
                }
            }
        }
        let key = plain_key(byte);
        out.push(key);
        if key == Key::Quit {
            return out;
        }
        index += 1;
    }
    out
}

/// Decodes one escape sequence, returning the key and the next index.
fn escape_key(input: &[u8], index: usize) -> Option<(Key, usize)> {
    if index + 2 >= input.len() || input[index + 1] != b'[' {
        return None;
    }
    match input[index + 2] {
        b'A' => Some((Key::Up, index + 3)),
        b'B' => Some((Key::Down, index + 3)),
        b'5' if index + 3 < input.len() && input[index + 3] == b'~' => Some((Key::Up, index + 4)),
        b'6' if index + 3 < input.len() && input[index + 3] == b'~' => Some((Key::Down, index + 4)),
        _ => None,
    }
}

/// Decodes one ordinary byte.
fn plain_key(byte: u8) -> Key {
    match byte {
        b'q' | b'Q' => Key::Quit,
        0x03 => Key::Quit,
        b'r' | b'R' => Key::Refresh,
        b'j' | b'J' => Key::Down,
        b'k' | b'K' => Key::Up,
        b'+' | b'=' | b'p' => Key::IntervalUp,
        b'-' | b'm' => Key::IntervalDown,
        b'o' | b'O' => Key::Open,
        _ => Key::None,
    }
}

/// Decodes one line of the line-based reader.
pub fn decode_line(line: &str) -> Key {
    match line {
        "q" | "Q" | "\u{1b}" => Key::Quit,
        "r" | "R" => Key::Refresh,
        "j" | "J" | "\u{1b}[B" => Key::Down,
        "k" | "K" | "\u{1b}[A" => Key::Up,
        "+" | "=" | "p" => Key::IntervalUp,
        "-" | "m" => Key::IntervalDown,
        "o" | "O" => Key::Open,
        _ => Key::None,
    }
}

#[cfg(test)]
mod tests;
