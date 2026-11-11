// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Turning a flag's textual default back into the spelling Go prints.

use super::FlagValue;

/// `time.Duration.String` for the values a flag default can hold: whole
/// seconds first, then the smaller units, with the sign out front.
pub fn duration_string(nanos: i64) -> String {
    if nanos == 0 {
        return "0s".to_string();
    }
    let sign = if nanos < 0 { "-" } else { "" };
    let mut rest = nanos.unsigned_abs() as u128;
    let hours = rest / 3_600_000_000_000;
    rest %= 3_600_000_000_000;
    let minutes = rest / 60_000_000_000;
    rest %= 60_000_000_000;
    let mut out = String::new();
    if hours > 0 {
        out.push_str(&format!("{hours}h"));
    }
    if minutes > 0 {
        out.push_str(&format!("{minutes}m"));
    }
    if hours > 0 || minutes > 0 || rest >= 1_000_000_000 {
        out.push_str(&seconds_part(rest));
    } else {
        out.push_str(&sub_second_part(rest));
    }
    format!("{sign}{out}")
}

/// The `…s` part of a duration that is at least one second long.
fn seconds_part(rest: u128) -> String {
    let secs = rest / 1_000_000_000;
    let frac = rest % 1_000_000_000;
    if frac == 0 {
        return format!("{secs}s");
    }
    let digits = format!("{frac:09}");
    format!("{}.{}s", secs, digits.trim_end_matches('0'))
}

/// The `ms` / `us` / `ns` part of a sub-second duration.
fn sub_second_part(rest: u128) -> String {
    if rest >= 1_000_000 {
        return format!("{}ms", rest / 1_000_000);
    }
    if rest >= 1_000 {
        return format!("{}us", rest / 1_000);
    }
    format!("{rest}ns")
}

/// `strconv.ParseBool`: exactly the spellings Go's boolean flags accept.
pub fn parse_go_bool(raw: &str) -> Option<bool> {
    match raw {
        "1" | "t" | "T" | "TRUE" | "true" | "True" => Some(true),
        "0" | "f" | "F" | "FALSE" | "false" | "False" => Some(false),
        _ => None,
    }
}

/// `strconv.FormatFloat(v, 'g', -1, 64)`, the spelling Go's `flag` records
/// as a float flag's `DefValue`.
///
/// The shortest round-tripping form is what Rust's `Display` already prints;
/// Go switches to the exponent form once the exponent leaves `[-4, 21)`, so
/// the two agree on every flag default a command actually registers.
pub fn format_go_float(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e21 {
        return format!("{value:.0}");
    }
    format!("{value}")
}

impl FlagValue {
    /// Returns the boolean payload, or `default` for other kinds.
    pub fn as_bool(&self, default: bool) -> bool {
        match self {
            FlagValue::Bool(v) => *v,
            _ => default,
        }
    }

    /// Returns the string payload, or `default` for other kinds.
    pub fn as_str(&self, default: &str) -> String {
        match self {
            FlagValue::Str(v) => v.clone(),
            _ => default.to_string(),
        }
    }

    /// Returns the integer payload, or `default` for other kinds.
    pub fn as_int(&self, default: i64) -> i64 {
        match self {
            FlagValue::Int(v) => *v,
            _ => default,
        }
    }

    /// Returns the duration payload in nanoseconds, or `default` for
    /// other kinds.
    pub fn as_duration(&self, default: i64) -> i64 {
        match self {
            FlagValue::Duration(v) => *v,
            _ => default,
        }
    }

    /// Returns the list payload, or an empty slice for other kinds.
    pub fn as_list(&self) -> &[String] {
        match self {
            FlagValue::List(v) => v.as_slice(),
            _ => &[],
        }
    }

    /// Returns the floating-point payload, or `default` for other kinds.
    pub fn as_float(&self, default: f64) -> f64 {
        match self {
            FlagValue::Float(v) => *v,
            _ => default,
        }
    }

    /// Returns the accumulated parts of a repeatable value flag.
    pub fn as_values(&self) -> &[String] {
        match self {
            FlagValue::Values(v) => v.as_slice(),
            _ => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spells_durations_the_way_go_does() {
        assert_eq!(duration_string(0), "0s");
        assert_eq!(duration_string(-1_500_000_000), "-1.5s");
        assert_eq!(duration_string(1_000_000), "1ms");
        assert_eq!(duration_string(1_000), "1us");
        assert_eq!(duration_string(7), "7ns");
        assert_eq!(duration_string(3_930_000_000_000), "1h5m30s");
    }

    #[test]
    fn only_go_s_boolean_spellings_parse() {
        assert_eq!(parse_go_bool("TRUE"), Some(true));
        assert_eq!(parse_go_bool("0"), Some(false));
        assert_eq!(parse_go_bool("yes"), None);
    }
}
