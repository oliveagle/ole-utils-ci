// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `strconv.ParseInt` and `strconv.ParseFloat`.

use super::{split_sign, underscore_ok, NumError};

/// `strconv.ParseInt(s, 0, 64)`: the base comes from the prefix.
pub fn parse_int(raw: &str) -> Result<i64, NumError> {
    if raw.is_empty() {
        return Err(NumError::Syntax);
    }
    let (negative, body) = split_sign(raw);
    let magnitude = parse_uint(body)?;
    const CUTOFF: u64 = 1 << 63;
    if !negative && magnitude >= CUTOFF {
        return Err(NumError::Range);
    }
    if negative && magnitude > CUTOFF {
        return Err(NumError::Range);
    }
    if negative && magnitude == CUTOFF {
        return Ok(i64::MIN);
    }
    let value = magnitude as i64;
    Ok(if negative { -value } else { value })
}

/// The unsigned half of [`parse_int`], including base detection.
fn parse_uint(raw: &str) -> Result<u64, NumError> {
    if raw.is_empty() {
        return Err(NumError::Syntax);
    }
    let (base, digits) = split_base(raw);
    let cutoff = u64::MAX / base + 1;
    let mut underscores = false;
    let mut value: u64 = 0;
    for &byte in digits.as_bytes() {
        if byte == b'_' {
            underscores = true;
            continue;
        }
        value = push_digit(value, byte, base, cutoff)?;
    }
    if underscores && !underscore_ok(raw) {
        return Err(NumError::Syntax);
    }
    Ok(value)
}

/// Splits Go's `0b` / `0o` / `0x` prefixes off `raw`.
fn split_base(raw: &str) -> (u64, &str) {
    if raw.as_bytes()[0] != b'0' {
        return (10, raw);
    }
    let marker = raw.as_bytes().get(1).map(u8::to_ascii_lowercase);
    if raw.len() >= 3 {
        match marker {
            Some(b'b') => return (2, &raw[2..]),
            Some(b'o') => return (8, &raw[2..]),
            Some(b'x') => return (16, &raw[2..]),
            _ => {}
        }
    }
    (8, &raw[1..])
}

/// Folds one digit into `value`, refusing anything that leaves the base or
/// overflowed `u64`.
fn push_digit(value: u64, byte: u8, base: u64, cutoff: u64) -> Result<u64, NumError> {
    let digit = if byte.is_ascii_digit() {
        u64::from(byte - b'0')
    } else if byte.is_ascii_alphabetic() {
        u64::from(byte.to_ascii_lowercase() - b'a' + 10)
    } else {
        return Err(NumError::Syntax);
    };
    if digit >= base {
        return Err(NumError::Syntax);
    }
    if value >= cutoff {
        return Err(NumError::Range);
    }
    let scaled = value * base;
    let next = scaled + digit;
    if next < scaled {
        return Err(NumError::Range);
    }
    Ok(next)
}

/// `strconv.ParseFloat(s, 64)`, including the hex-float extension.
pub fn parse_float(raw: &str) -> Result<f64, NumError> {
    if raw.is_empty() {
        return Err(NumError::Syntax);
    }
    if raw.contains('_') && !underscore_ok(raw) {
        return Err(NumError::Syntax);
    }
    let (negative, body) = split_sign(raw);
    if is_special(body) {
        return raw.parse::<f64>().map_err(|_| NumError::Syntax);
    }
    if let Some(mantissa) = strip_hex_prefix(body) {
        return parse_hex_float(negative, mantissa);
    }
    match raw.replace('_', "").parse::<f64>() {
        Ok(value) if value.is_infinite() => Err(NumError::Range),
        Ok(value) => Ok(value),
        Err(_) => Err(NumError::Syntax),
    }
}

/// Whether `body` is one of Go's special floating-point words.
fn is_special(body: &str) -> bool {
    body.eq_ignore_ascii_case("inf")
        || body.eq_ignore_ascii_case("infinity")
        || body.eq_ignore_ascii_case("nan")
}

/// Drops a leading `0x` / `0X`, which only introduces a hex float.
fn strip_hex_prefix(body: &str) -> Option<&str> {
    body.strip_prefix("0x").or_else(|| body.strip_prefix("0X"))
}

/// Parses the mantissa and binary exponent of a hex float.
fn parse_hex_float(negative: bool, rest: &str) -> Result<f64, NumError> {
    let Some(split) = rest.find(['p', 'P']) else {
        return Err(NumError::Syntax);
    };
    let (mantissa, exponent) = (&rest[..split], &rest[split + 1..]);
    if mantissa.is_empty() || exponent.is_empty() {
        return Err(NumError::Syntax);
    }
    let (value, digits) = scan_mantissa(mantissa)?;
    if digits == 0 {
        return Err(NumError::Syntax);
    }
    if value == 0.0 {
        return Ok(if negative { -0.0 } else { 0.0 });
    }
    let scaled = value * 2f64.powi(parse_exponent(exponent)?);
    if scaled.is_infinite() {
        return Err(NumError::Range);
    }
    Ok(if negative { -scaled } else { scaled })
}

/// Accumulates a hex mantissa, returning its value and its digit count.
fn scan_mantissa(mantissa: &str) -> Result<(f64, usize), NumError> {
    let mut value = 0.0f64;
    let mut scale = 1.0f64;
    let mut fractional = false;
    let mut digits = 0usize;
    for byte in mantissa.bytes() {
        if byte == b'_' {
            continue;
        }
        if byte == b'.' {
            if fractional {
                return Err(NumError::Syntax);
            }
            fractional = true;
            continue;
        }
        let digit = hex_digit(byte).ok_or(NumError::Syntax)?;
        digits += 1;
        if fractional {
            scale /= 16.0;
            value += digit * scale;
        } else {
            value = value * 16.0 + digit;
        }
    }
    Ok((value, digits))
}

/// The `p` exponent of a hex float, clamped to where it still matters.
fn parse_exponent(raw: &str) -> Result<i32, NumError> {
    let (negative, body) = split_sign(raw);
    if body.is_empty() || !body.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(NumError::Syntax);
    }
    let magnitude: i64 = body.parse().map_err(|_| NumError::Range)?;
    let value = if negative { -magnitude } else { magnitude };
    Ok(value.clamp(-4_000, 4_000) as i32)
}

/// One hexadecimal digit as a `f64`.
fn hex_digit(byte: u8) -> Option<f64> {
    let digit = match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => return None,
    };
    Some(f64::from(digit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_pick_their_base_from_the_prefix() {
        assert_eq!(parse_int("16"), Ok(16));
        assert_eq!(parse_int("0x10"), Ok(16));
        assert_eq!(parse_int("0b101"), Ok(5));
        assert_eq!(parse_int("0o17"), Ok(15));
        assert_eq!(parse_int("010"), Ok(8));
        assert_eq!(parse_int("1_000"), Ok(1_000));
        assert_eq!(parse_int("+5"), Ok(5));
    }

    #[test]
    fn integers_reject_what_go_rejects() {
        assert_eq!(parse_int("08"), Err(NumError::Syntax));
        assert_eq!(parse_int("abc"), Err(NumError::Syntax));
        assert_eq!(parse_int("99999999999999999999"), Err(NumError::Range));
        assert_eq!(parse_int(""), Err(NumError::Syntax));
    }

    #[test]
    fn floats_accept_decimal_hex_and_special_words() {
        assert_eq!(parse_float("95"), Ok(95.0));
        assert_eq!(parse_float("1_0"), Ok(10.0));
        assert_eq!(parse_float("08"), Ok(8.0));
        assert_eq!(parse_float(".5"), Ok(0.5));
        assert_eq!(parse_float("0x10p0"), Ok(16.0));
        assert_eq!(parse_float("0X1P+2"), Ok(4.0));
        assert!(parse_float("inf").unwrap().is_infinite());
        assert!(parse_float("Infinity").unwrap().is_infinite());
        assert!(parse_float("nan").unwrap().is_nan());
    }

    #[test]
    fn floats_reject_what_go_rejects() {
        assert_eq!(parse_float("0x10"), Err(NumError::Syntax));
        assert_eq!(parse_float("1e"), Err(NumError::Syntax));
        assert_eq!(parse_float("1p1"), Err(NumError::Syntax));
        assert_eq!(parse_float("1__0"), Err(NumError::Syntax));
        assert_eq!(parse_float("1e400"), Err(NumError::Range));
        assert_eq!(parse_float("1e-400"), Ok(0.0));
    }
}
