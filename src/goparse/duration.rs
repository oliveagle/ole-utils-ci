// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `time.ParseDuration`.

use super::split_sign;

/// `time.ParseDuration`, returning the duration in nanoseconds.
///
/// Go stores a `time.Duration` as a signed nanosecond count, so the sign is
/// kept rather than folded into a `std::time::Duration`.
pub fn parse_duration(raw: &str) -> Option<i64> {
    let (negative, mut rest) = split_sign(raw);
    if rest == "0" {
        return Some(0);
    }
    if rest.is_empty() {
        return None;
    }
    let mut total: u64 = 0;
    while !rest.is_empty() {
        let (value, remainder) = parse_segment(rest)?;
        rest = remainder;
        total = total.checked_add(value)?;
        if total > (1u64 << 63) {
            return None;
        }
    }
    signed(total, negative)
}

/// Folds the sign back into the accumulated magnitude.
fn signed(total: u64, negative: bool) -> Option<i64> {
    if !negative {
        return (total < (1u64 << 63)).then_some(total as i64);
    }
    if total == 1u64 << 63 {
        return Some(i64::MIN);
    }
    Some(-(total as i64))
}

/// Parses one `number unit` term and returns its nanoseconds and the rest.
fn parse_segment(raw: &str) -> Option<(u64, &str)> {
    let (leading, remainder) = leading_int(raw)?;
    let pre = remainder.len() != raw.len();
    let (fraction, scale, rest, post) = split_fraction(remainder);
    if !pre && !post {
        return None;
    }
    let unit_len = rest
        .char_indices()
        .find(|(_, c)| *c == '.' || c.is_ascii_digit())
        .map(|(index, _)| index)
        .unwrap_or(rest.len());
    if unit_len == 0 {
        return None;
    }
    let unit = unit_nanos(&rest[..unit_len])?;
    let rest = &rest[unit_len..];
    if leading > (1u64 << 63) / unit {
        return None;
    }
    let mut value = leading * unit;
    if fraction > 0 {
        let add = (fraction as f64) * (unit as f64 / scale);
        value = value.saturating_add(add as u64);
        if value > (1u64 << 63) {
            return None;
        }
    }
    Some((value, rest))
}

/// Splits the optional fractional part off a term.
///
/// The flag reports whether at least one digit followed the dot, which is
/// what separates `5.s` (accepted) from `.s` (rejected).
fn split_fraction(raw: &str) -> (u64, f64, &str, bool) {
    let Some(after_dot) = raw.strip_prefix('.') else {
        return (0, 1.0, raw, false);
    };
    let (digits, divisor, remainder) = leading_fraction(after_dot);
    (
        digits,
        divisor,
        remainder,
        remainder.len() != after_dot.len(),
    )
}

/// `time`'s `leadingInt`: the digits before the unit.
fn leading_int(raw: &str) -> Option<(u64, &str)> {
    let mut value: u64 = 0;
    let mut end = 0;
    for (index, byte) in raw.bytes().enumerate() {
        if !byte.is_ascii_digit() {
            break;
        }
        if value > (1u64 << 63) / 10 {
            return None;
        }
        value = value * 10 + u64::from(byte - b'0');
        if value > (1u64 << 63) {
            return None;
        }
        end = index + 1;
    }
    Some((value, &raw[end..]))
}

/// `time`'s `leadingFraction`: the digits after the decimal point.
fn leading_fraction(raw: &str) -> (u64, f64, &str) {
    let mut value: u64 = 0;
    let mut scale = 1.0f64;
    let mut overflow = false;
    let mut end = 0;
    for (index, byte) in raw.bytes().enumerate() {
        if !byte.is_ascii_digit() {
            break;
        }
        end = index + 1;
        if overflow {
            continue;
        }
        if value > ((1u64 << 63) - 1) / 10 {
            overflow = true;
            continue;
        }
        let next = value * 10 + u64::from(byte - b'0');
        if next > (1u64 << 63) {
            overflow = true;
            continue;
        }
        value = next;
        scale *= 10.0;
    }
    (value, scale, &raw[end..])
}

/// The nanosecond count of one duration unit, or `None` if unknown.
fn unit_nanos(unit: &str) -> Option<u64> {
    match unit {
        "ns" => Some(1),
        "us" | "\u{b5}s" | "\u{3bc}s" => Some(1_000),
        "ms" => Some(1_000_000),
        "s" => Some(1_000_000_000),
        "m" => Some(60_000_000_000),
        "h" => Some(3_600_000_000_000),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_spellings_go_accepts() {
        for good in [
            "0", "-0", "+0", "0s", "1.5s", "1m30s", "100ms", "1us", "1µs", "1μs", "2h45m", "+5s",
            "-1s", ".5s", "5.s", "1ns", "1ns1ns", "1m0.5s",
        ] {
            assert!(parse_duration(good).is_some(), "{good} should parse");
        }
    }

    #[test]
    fn rejects_the_spellings_go_rejects() {
        for bad in [
            "5",
            "1.5",
            "1e3",
            "1_0",
            "0x10",
            "1y",
            "1M",
            " 0",
            "0 ",
            "-",
            "+",
            "1s2",
            "999999999999999999999s",
        ] {
            assert!(parse_duration(bad).is_none(), "{bad} should not parse");
        }
    }

    #[test]
    fn keeps_the_signed_nanosecond_representation() {
        assert_eq!(parse_duration("1s"), Some(1_000_000_000));
        assert_eq!(parse_duration("-1s"), Some(-1_000_000_000));
        assert_eq!(parse_duration("2562047h47m16.854775807s"), Some(i64::MAX));
        assert_eq!(parse_duration("2562047h47m16.854775808s"), None);
    }
}
