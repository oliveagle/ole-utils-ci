// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Timestamp formatting that matches Go's `encoding/json`.
//!
//! Go marshals a `time.Time` as RFC3339 with the fractional part trimmed
//! of trailing zeros and a `Z` for UTC. `chrono`'s own serializer keeps
//! the fraction at three digits and appends `+00:00`, so the two disagree
//! on every stamp that carries milliseconds — which is every stamp GitLab
//! sends. The formatter itself lives in [`crate::gojson`], which
//! `repo-status` shares.

use chrono::{DateTime, Utc};
use serde::Serializer;

/// Formats an optional timestamp the way Go's `encoding/json` does.
pub fn go_time<S>(value: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    crate::gojson::serialize_stamp(value, serializer)
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
    crate::gojson::rfc3339_nano(stamp)
}

#[cfg(test)]
mod tests;
