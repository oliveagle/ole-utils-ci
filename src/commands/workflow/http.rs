// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The two token-free polls the workflow relies on: an anonymous HEAD against
//! artifactory, and a GET against an app's `/version` endpoint.
//!
//! Both take their poll step as a parameter so tests never wait for the
//! production cadence (20s for artifactory, 5s for `/version`).

use std::time::{Duration, Instant};

use crate::httpd;

use super::util::first_chars;

/// The step the artifactory poll waits between HEADs.
pub const ARTIFACTORY_POLL_STEP: Duration = Duration::from_secs(20);

/// The step the `/version` poll waits between GETs.
pub const APP_VERSION_POLL_STEP: Duration = Duration::from_secs(5);

/// The result of one HTTP HEAD.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeadInfo {
    /// The status code, `0` when the request never got an answer.
    pub code: u16,
    /// `Content-Length`, `0` when absent or unparseable.
    pub len: i64,
    /// Whether the artifact answered `200`.
    pub present: bool,
}

/// The outcome of one `/version` GET.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VersionProbe {
    /// The `app_version` the endpoint reported.
    pub got: String,
    /// The status code, `0` on a transport failure.
    pub status: u16,
    /// Why the probe failed, when it did.
    pub error: Option<String>,
}

/// One anonymous HEAD with a short timeout.
pub fn http_head(url: &str) -> HeadInfo {
    match httpd::request("HEAD", url, &[], None, Duration::from_secs(15)) {
        Ok(response) => HeadInfo {
            code: response.status,
            len: response
                .header("Content-Length")
                .and_then(|raw| raw.trim().parse::<i64>().ok())
                .unwrap_or(0),
            present: response.status == 200,
        },
        Err(_) => HeadInfo::default(),
    }
}

/// Polls `url` until HEAD answers 200 with at least `min_size_b` bytes.
///
/// This is the token-free verification `release-rc.sh` uses: an artifact on
/// artifactory means the Upload job succeeded.
pub fn poll_artifactory(
    label: &str,
    url: &str,
    min_size_b: i64,
    timeout_sec: i64,
    verbose: bool,
    step: Duration,
) -> Result<HeadInfo, String> {
    let deadline = Instant::now() + seconds(timeout_sec);
    loop {
        let last = http_head(url);
        if last.present && last.len >= min_size_b {
            return Ok(last);
        }
        if Instant::now() > deadline {
            return Err(format!(
                "{label} artifact not live after {timeout_sec}s: HEAD {url} → HTTP {} len={} \
                 (need 200, >={min_size_b} B)",
                last.code, last.len
            ));
        }
        if verbose {
            println!(
                "  [{label}] HEAD {url} → HTTP {} len={} (retrying in {}s)",
                last.code,
                last.len,
                step.as_secs()
            );
        }
        std::thread::sleep(step);
    }
}

/// One GET of an app's `/version` endpoint.
pub fn probe_app_version(url: &str) -> VersionProbe {
    let response = match httpd::request("GET", url, &[], None, Duration::from_secs(10)) {
        Ok(response) => response,
        Err(error) => {
            return VersionProbe {
                error: Some(error),
                ..VersionProbe::default()
            }
        }
    };
    let body = &response.body[..response.body.len().min(1 << 20)];
    let parsed: serde_json::Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(error) => {
            return VersionProbe {
                got: String::new(),
                status: response.status,
                error: Some(format!(
                    "parse /version {url}: {error} (body: {})",
                    first_chars(&response.body_string(), 120)
                )),
            }
        }
    };
    VersionProbe {
        got: parsed
            .get("app_version")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string(),
        status: response.status,
        error: None,
    }
}

/// Polls `/version` until `app_version` equals `want` (`""` accepts any
/// non-empty version), or the timeout elapses.
pub fn poll_app_version(
    url: &str,
    want: &str,
    timeout_sec: i64,
    verbose: bool,
    step: Duration,
) -> Result<String, String> {
    let deadline = Instant::now() + seconds(timeout_sec);
    loop {
        let probe = probe_app_version(url);
        let matched = probe.error.is_none()
            && !probe.got.is_empty()
            && (want.is_empty() || probe.got == want);
        if matched {
            return Ok(probe.got);
        }
        if Instant::now() > deadline {
            return Err(format!(
                "app /version {url} never reached {want:?} (last: {:?} HTTP {}) after {timeout_sec}s",
                probe.got, probe.status
            ));
        }
        if verbose {
            println!(
                "  [{url}] /version → app_version={:?} (want {want:?}) HTTP {}",
                probe.got, probe.status
            );
        }
        std::thread::sleep(step);
    }
}

/// A non-negative second count as a [`Duration`].
fn seconds(count: i64) -> Duration {
    Duration::from_secs(count.max(0) as u64)
}

#[cfg(test)]
mod tests;
