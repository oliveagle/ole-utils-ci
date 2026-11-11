// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Turning a flag's textual default back into the spelling Go prints.

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
