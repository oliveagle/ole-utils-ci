// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Timestamp formatting that matches Go's `encoding/json`.
//!
//! Go marshals a `time.Time` as RFC3339 with the fractional part trimmed
//! of trailing zeros and a `Z` for UTC. `chrono`'s own serializer keeps
//! the fraction at three digits and appends `+00:00`, so the two disagree
//! on every stamp that carries milliseconds — which is every stamp GitLab
//! sends.

use chrono::{DateTime, Utc};
use serde::Serializer;

/// The timestamp Go prints for an unset `time.Time`.
pub const ZERO_TIME: &str = "0001-01-01T00:00:00Z";

/// Formats an optional timestamp the way Go's `encoding/json` does.
pub fn go_time<S>(value: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let text = match value {
        Some(stamp) => rfc3339_nano(stamp),
        None => ZERO_TIME.to_string(),
    };
    serializer.serialize_str(&text)
}

/// Formats a timestamp the way Go's `encoding/json` does.
pub fn go_stamp<S>(stamp: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&rfc3339_nano(stamp))
}

/// RFC3339 with the nanosecond fraction trimmed, like Go's RFC3339Nano.
pub fn rfc3339_nano(stamp: &DateTime<Utc>) -> String {
    let head = stamp.format("%Y-%m-%dT%H:%M:%S").to_string();
    let nanos = stamp.timestamp_subsec_nanos();
    if nanos == 0 {
        return format!("{head}Z");
    }
    let mut fraction = format!("{nanos:09}");
    while fraction.ends_with('0') {
        fraction.pop();
    }
    format!("{head}.{fraction}Z")
}

#[cfg(test)]
mod tests;
