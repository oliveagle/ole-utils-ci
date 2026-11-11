// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `METADATA_DOMAIN` check: is the stub the tests need answering?
//!
//! `foundation.init()` reads one JSON document from this domain at package
//! start, so an unset, unreachable or unhappy endpoint is the difference
//! between a green test run and a panic.

use std::time::Duration;

use serde::Deserialize;

use super::checks::{get, Check};

/// The payload `foundation.init()` unmarshals.
#[derive(Debug, Deserialize)]
struct Metadata {
    /// `0` means "reachable and healthy"; an absent key and a `null` both
    /// read as `0`, the way Go's `json.Unmarshal` leaves an `int` alone
    /// for either. A body that is not JSON fails with `serde_json`'s
    /// wording rather than `encoding/json`'s, the other half of that
    /// divergence.
    #[serde(default)]
    status: Option<i64>,
}

/// `checkMetadataDomain`: unset is a warning, an endpoint that cannot be
/// read is a failure.
pub(super) fn metadata_domain() -> Vec<Check> {
    let domain = std::env::var("METADATA_DOMAIN").unwrap_or_default();
    if domain.is_empty() {
        return vec![Check::warn(
            "METADATA_DOMAIN",
            "METADATA_DOMAIN is not set — foundation.init() may panic if corp network is unavailable",
            "Start devine-utils-ci fake-metadata-server and set export METADATA_DOMAIN=http://localhost:18080 before running tests",
        )];
    }
    let mut results = vec![Check::pass(
        "METADATA_DOMAIN",
        format!("METADATA_DOMAIN={domain}"),
    )];
    let url = format!("{}/api/ctrip/metadata.json", domain.trim_end_matches('/'));
    results.push(endpoint(&url));
    results
}

/// The one probe of the endpoint itself.
fn endpoint(url: &str) -> Check {
    let response = match get(url, Duration::from_secs(3)) {
        Ok(response) => response,
        Err(error) => {
            return Check::fail(
                "  Metadata endpoint",
                format!("Cannot reach {url}: {error}"),
                "Start the fake metadata server: devine-utils-ci fake-metadata-server",
            );
        }
    };
    match payload_status(&response.body) {
        Ok(0) => Check::pass(
            "  Metadata endpoint",
            "Responded with status=0 — foundation.init() should succeed",
        ),
        Ok(status) => Check::warn(
            "  Metadata endpoint",
            format!("Responded with status={status}"),
            "",
        ),
        Err(error) => Check::fail(
            "  Metadata endpoint",
            format!("Response is not valid JSON: {error}"),
            r#"Ensure the metadata server returns {"status":0,...}"#,
        ),
    }
}

/// The `status` field, with the two leniencies `json.Unmarshal` has: a
/// body of `null`, and a key that is missing or `null`, both read as `0`.
pub(super) fn payload_status(body: &[u8]) -> Result<i64, String> {
    if String::from_utf8_lossy(body).trim() == "null" {
        return Ok(0);
    }
    let payload: Metadata = serde_json::from_slice(body).map_err(|error| error.to_string())?;
    Ok(payload.status.unwrap_or(0))
}
