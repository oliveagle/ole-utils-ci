// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The ANSI vocabulary of the dashboard.
//!
//! The Go original hand-rolls the same escape codes to avoid pulling in a
//! fat TUI library. The palette is 24-bit truecolor: the eight-colour
//! backgrounds are too dim on most terminals to read a white digit on top.

/// The reset sequence.
pub const RESET: &str = "\u{1b}[0m";
/// The bold sequence.
pub const BOLD: &str = "\u{1b}[1m";
/// The dim sequence.
pub const DIM: &str = "\u{1b}[2m";
/// The reverse-video sequence.
pub const REVERSE: &str = "\u{1b}[7m";
/// The bright white foreground.
pub const WHITE_FG: &str = "\u{1b}[38;2;255;255;255m";
/// The cyan foreground.
pub const CYAN: &str = "\u{1b}[36m";
/// The yellow foreground.
pub const YELLOW: &str = "\u{1b}[33m";

/// The selection background: a dark cool blue that spans the whole row.
pub const SEL_BG: &str = "\u{1b}[48;2;30;58;95m";
/// The red ALIGN badge: a red fill with a bold white foreground.
pub const ALIGN_RED_BADGE: &str = "\u{1b}[48;2;200;50;50m\u{1b}[38;2;255;255;255m\u{1b}[1m";

/// The glyph of a starred repo.
pub const STAR: &str = "\u{2605}";

/// The truecolor triple of a pipeline status.
///
/// One mapping serves both the background of the strip and the foreground
/// of the merge-request dot, so the operator memorises one vocabulary.
fn status_rgb(status: &str) -> (u8, u8, u8) {
    match status.trim().to_lowercase().as_str() {
        "success" => (46, 160, 67),
        "failed" => (200, 50, 50),
        "running" => (56, 139, 253),
        "warning" => (220, 180, 40),
        "pending" => (220, 180, 40),
        "canceled" | "cancelled" => (140, 80, 180),
        "skipped" => (130, 130, 170),
        _ => (110, 110, 110),
    }
}

/// The background colour of a pipeline status.
pub fn dot_bg(status: &str) -> String {
    let (red, green, blue) = status_rgb(status);
    format!("\u{1b}[48;2;{red};{green};{blue}m")
}

/// The foreground colour of a pipeline status.
pub fn dot_fg(status: &str) -> String {
    let (red, green, blue) = status_rgb(status);
    format!("\u{1b}[38;2;{red};{green};{blue}m")
}

/// The number of printable characters in `text`.
///
/// ANSI escape sequences do not consume width.
pub fn visible_length(text: &str) -> usize {
    let mut count = 0;
    let mut in_escape = false;
    for ch in text.chars() {
        if ch == '\u{1b}' {
            in_escape = true;
            continue;
        }
        if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }
        count += 1;
    }
    count
}

/// Trims `text` to `max_width` visible characters.
///
/// Any trailing reset is preserved so the terminal does not leak colour.
pub fn truncate_visible(text: &str, max_width: usize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut visible = 0;
    let mut in_escape = false;
    for ch in text.chars() {
        if visible >= max_width {
            break;
        }
        if ch == '\u{1b}' {
            in_escape = true;
            out.push(ch);
            continue;
        }
        if in_escape {
            out.push(ch);
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }
        out.push(ch);
        visible += 1;
    }
    out.push_str(RESET);
    out
}

#[cfg(test)]
mod tests;
