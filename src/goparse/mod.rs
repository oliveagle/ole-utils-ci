// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The numeric grammars Go's `strconv` and `time` accept.
//!
//! The `tengo-*` flags are typed the way Go's `flag` package types them, so
//! their values have to be rejected for exactly the same reasons Go rejects
//! them: base-0 integers, `ParseFloat`'s hex-float extension and
//! `time.ParseDuration`'s unit grammar.

mod duration;
mod scalar;

pub use duration::parse_duration;
pub use scalar::{parse_float, parse_int};

/// Why a Go numeric literal was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumError {
    /// The literal does not match Go's grammar.
    Syntax,
    /// The literal is well formed but does not fit the target type.
    Range,
}

impl NumError {
    /// The wording `strconv` puts after the value in a flag error.
    pub fn as_str(self) -> &'static str {
        match self {
            NumError::Syntax => "parse error",
            NumError::Range => "value out of range",
        }
    }
}

/// Splits an optional sign, returning whether the value was negative.
pub(super) fn split_sign(raw: &str) -> (bool, &str) {
    if let Some(rest) = raw.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = raw.strip_prefix('+') {
        (false, rest)
    } else {
        (false, raw)
    }
}

/// `strconv.underscoreOK`: an underscore must sit between digits, or between
/// a base prefix and a digit.
pub(super) fn underscore_ok(raw: &str) -> bool {
    let (_, body) = split_sign(raw);
    let bytes = body.as_bytes();
    let mut index = 0;
    let mut saw = Saw::Start;
    let mut hex = false;
    if bytes.len() >= 2
        && bytes[0] == b'0'
        && matches!(bytes[1].to_ascii_lowercase(), b'b' | b'o' | b'x')
    {
        index = 2;
        saw = Saw::Digit;
        hex = bytes[1].eq_ignore_ascii_case(&b'x');
    }
    while index < bytes.len() {
        let byte = bytes[index];
        let digit =
            byte.is_ascii_digit() || (hex && matches!(byte.to_ascii_lowercase(), b'a'..=b'f'));
        if digit {
            saw = Saw::Digit;
        } else if byte == b'_' {
            if saw != Saw::Digit {
                return false;
            }
            saw = Saw::Under;
        } else {
            if saw == Saw::Under {
                return false;
            }
            saw = Saw::Other;
        }
        index += 1;
    }
    saw != Saw::Under
}

/// What [`underscore_ok`] has seen so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Saw {
    /// The beginning of the number.
    Start,
    /// A digit, or a base prefix that counts as one.
    Digit,
    /// An underscore, which must be followed by a digit.
    Under,
    /// Anything else, which ends any run.
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn underscores_must_separate_digits() {
        assert!(underscore_ok("1_0"));
        assert!(underscore_ok("0x1_0"));
        assert!(underscore_ok("0b1_1"));
        assert!(!underscore_ok("_1"));
        assert!(!underscore_ok("1_"));
        assert!(!underscore_ok("1__0"));
    }

    #[test]
    fn a_base_prefix_counts_as_a_digit() {
        assert!(underscore_ok("0x_1"));
        assert!(underscore_ok("0b_1"));
        assert!(!underscore_ok("0x1_p0"));
    }

    #[test]
    fn signs_are_stripped_before_the_scan() {
        assert!(underscore_ok("-1_0"));
        assert!(!underscore_ok("+_1"));
    }
}
