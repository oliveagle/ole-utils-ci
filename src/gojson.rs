// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The JSON quirks Go's `encoding/json` has and serde_json does not.
//!
//! The Rust port has to stay byte-compatible with the Go tool wherever the
//! two print the same document. Go escapes the HTML-significant bytes inside
//! every string and writes `\u0008`/`\u000c` where serde_json writes
//! `\b`/`\f`. The helpers here rewrite a serde_json document the way Go
//! would have written it.

/// Rewrites a serde_json document the way Go's `encoding/json` would.
///
/// The scan walks the document escape by escape, so a literal backslash in
/// the data is never mistaken for the start of an escape sequence.
///
/// `<`, `>`, `&`, U+2028 and U+2029 can only occur inside a JSON string, so
/// replacing them in place is exact.
pub fn to_go_json(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    let mut chars = json.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '&' => out.push_str("\\u0026"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            '\\' => {
                out.push('\\');
                match chars.next() {
                    Some('b') => out.push_str("u0008"),
                    Some('f') => out.push_str("u000c"),
                    Some(next) => out.push(next),
                    None => {}
                }
            }
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests;

/// Formats a float the way Go's `encoding/json` writes one.
///
/// Go drops the fractional part of an integral value (`600`, not `600.0`)
/// and switches to `e` notation — with an explicit sign and at least two
/// digits — outside the `1e-6 .. 1e21` window.
pub fn go_float(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    let absolute = value.abs();
    if absolute != 0.0 && (absolute < 1e-6 || absolute >= 1e21) {
        return go_exponent_form(value);
    }
    // Rust's `Display` keeps the shortest round-tripping digits in
    // positional notation, which is what `strconv.AppendFloat(.., 'f', -1,
    // 64)` writes.
    format!("{value}")
}

/// The `e` notation Go falls back to outside the plain-decimal window.
fn go_exponent_form(value: f64) -> String {
    let text = format!("{value:e}");
    let Some((mantissa, exponent)) = text.split_once('e') else {
        return text;
    };
    let Ok(parsed) = exponent.parse::<i32>() else {
        return text;
    };
    let sign = if parsed < 0 { '-' } else { '+' };
    let digits = parsed.abs();
    // Go writes at least two exponent digits, then trims a single negative
    // digit back to one.
    let exponent_text = if digits < 10 && parsed < 0 {
        format!("{digits}")
    } else {
        format!("{digits:02}")
    };
    format!("{mantissa}e{sign}{exponent_text}")
}

/// Serializes a float the way Go's `encoding/json` writes one.
///
/// serde_json always keeps the fractional part of an integral float, so the
/// number is written as a raw JSON fragment instead.
pub fn serialize_float<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let raw = serde_json::value::RawValue::from_string(go_float(*value))
        .map_err(serde::ser::Error::custom)?;
    serde::Serialize::serialize(raw.as_ref(), serializer)
}
