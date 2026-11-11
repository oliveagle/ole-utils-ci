// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the Go-compatible timestamp formatting.

use chrono::{TimeZone, Timelike, Utc};
use serde::Serialize;

use super::*;

/// A holder that serializes its stamp the way the rows do.
#[derive(Serialize)]
struct Holder {
    #[serde(serialize_with = "go_time")]
    stamp: Option<DateTime<Utc>>,
}

#[test]
fn trims_the_fraction_like_go() {
    let stamp = Utc.with_ymd_and_hms(2026, 11, 11, 9, 12, 33).unwrap();
    assert_eq!(rfc3339_nano(&stamp), "2026-11-11T09:12:33Z");
}

#[test]
fn drops_the_trailing_zeros_of_the_fraction() {
    let stamp = Utc
        .with_ymd_and_hms(2026, 11, 11, 9, 12, 33)
        .unwrap()
        .with_nanosecond(123_000_000)
        .unwrap();
    assert_eq!(rfc3339_nano(&stamp), "2026-11-11T09:12:33.123Z");
}

#[test]
fn keeps_a_nine_digit_fraction() {
    let stamp = Utc
        .with_ymd_and_hms(2026, 11, 11, 9, 12, 33)
        .unwrap()
        .with_nanosecond(123_456_789)
        .unwrap();
    assert_eq!(rfc3339_nano(&stamp), "2026-11-11T09:12:33.123456789Z");
}

#[test]
fn an_unset_stamp_becomes_the_go_zero_time() {
    let text = serde_json::to_string(&Holder { stamp: None }).unwrap();
    assert_eq!(
        text,
        format!("{{\"stamp\":\"{}\"}}", crate::gojson::ZERO_TIME)
    );
}

#[test]
fn a_set_stamp_keeps_its_milliseconds() {
    let stamp = Utc
        .with_ymd_and_hms(2026, 11, 11, 9, 12, 33)
        .unwrap()
        .with_nanosecond(123_000_000)
        .unwrap();
    let text = serde_json::to_string(&Holder { stamp: Some(stamp) }).unwrap();
    assert_eq!(text, "{\"stamp\":\"2026-11-11T09:12:33.123Z\"}");
}
