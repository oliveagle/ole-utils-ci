// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The Captain release API, as far as `deploy-prod` needs it.
//!
//! Creating a release (`auto_deploy`) leaves it at PENDING; a rollout push
//! is what actually deploys it. Both write calls need a credential, and
//! Captain decides which header shape it is looking at: a UI session token
//! arrives as `X-User-Token`, an OAuth `access_token` from the faas
//! credential store as `Authorization: Bearer`. Sending both means the
//! caller does not have to know which one it holds.

use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::httpd;

mod model;

use model::{AutoDeployResponse, ReleasesEnvelope};

/// The Captain release API root.
pub const CAPTAIN_BASE_URL: &str = "https://captain.release.ctripcorp.com/faas/v1";

/// How often the release wait polls Captain.
pub const RELEASE_POLL_STEP: Duration = Duration::from_secs(15);

/// The subset of a Captain release record this tool reads.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CaptainRelease {
    /// The release id.
    #[serde(default)]
    pub id: i64,
    /// The release status (`PENDING`, `SUCCESS`, `FAILED`, ...).
    #[serde(default)]
    pub status: String,
    /// The fields the release changes; `Image URL` carries the image.
    #[serde(default)]
    pub diff: Vec<ReleaseDiff>,
}

/// One changed field of a release.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReleaseDiff {
    /// The field name.
    #[serde(default)]
    pub key: String,
    /// The value the release sets.
    #[serde(default)]
    pub to: String,
}

/// A read/write client for one Captain instance.
#[derive(Debug, Clone)]
pub struct CaptainClient {
    base: String,
    token: String,
}

impl CaptainClient {
    /// Builds a client; `base` ends before the resource path.
    pub fn new(base: impl Into<String>, token: impl Into<String>) -> CaptainClient {
        CaptainClient {
            base: base.into(),
            token: token.into(),
        }
    }

    /// Performs one API call and returns the raw body.
    pub fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<Vec<u8>, String> {
        let url = format!("{}{}", self.base, path);
        let payload = match body {
            Some(value) => Some(
                serde_json::to_vec(value)
                    .map_err(|error| format!("encode {method} {path}: {error}"))?,
            ),
            None => None,
        };
        let mut headers: Vec<(String, String)> =
            vec![("Content-Type".to_string(), "application/json".to_string())];
        if !self.token.is_empty() {
            headers.push((
                "Authorization".to_string(),
                format!("Bearer {}", self.token),
            ));
            headers.push(("X-User-Token".to_string(), self.token.clone()));
        }
        let borrowed: Vec<(&str, &str)> = headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        let response = httpd::request(
            method,
            &url,
            &borrowed,
            payload.as_deref(),
            Duration::from_secs(30),
        )
        .map_err(|error| format!("Captain {method} {path} → {error}"))?;
        if !(200..300).contains(&response.status) {
            return Err(format!(
                "Captain {method} {path} → HTTP {}: {}",
                response.status,
                super::util::first_chars(&response.body_string(), 300)
            ));
        }
        Ok(response.body)
    }

    /// The recent releases of one group.
    pub fn releases(&self, group_id: i64) -> Result<Vec<CaptainRelease>, String> {
        let raw = self.request(
            "GET",
            &format!("/releases?group_id={group_id}&per_page=10"),
            None,
        )?;
        let envelope: ReleasesEnvelope = serde_json::from_slice(&raw)
            .map_err(|error| format!("parse Captain releases: {error}"))?;
        Ok(envelope.result)
    }

    /// The image URL of the newest SUCCESS release on the FAT group — the
    /// exact image, timestamp suffix included.
    pub fn latest_fat_image(&self, fat_group: i64) -> Result<String, String> {
        let releases = self.releases(fat_group)?;
        for release in &releases {
            if release.status != "SUCCESS" {
                continue;
            }
            for diff in &release.diff {
                if diff.key == "Image URL" && !diff.to.is_empty() {
                    return Ok(diff.to.clone());
                }
            }
        }
        Err(format!(
            "no SUCCESS release with Image URL in FAT group {fat_group} (latest {} releases)",
            releases.len()
        ))
    }

    /// Creates a PROD release, which starts at PENDING.
    pub fn auto_deploy(&self, group_id: i64, image: &str) -> Result<i64, String> {
        // The keys are alphabetical, the way Go's map marshalling orders them.
        let body = serde_json::json!({
            "batches": [{"delay": 0, "percent": 100}],
            "id": group_id,
            "image_url": image,
            "resource": "RESOURCE_GROUP",
            "username": "rhtang",
        });
        let raw = self.request("POST", "/releases/auto_deploy", Some(&body))?;
        let parsed: AutoDeployResponse = serde_json::from_slice(&raw).map_err(|error| {
            format!(
                "parse auto_deploy response: {error} (body: {})",
                super::util::first_chars(&String::from_utf8_lossy(&raw), 200)
            )
        })?;
        let ids = [
            parsed.id,
            parsed.release_id,
            parsed.release.id,
            parsed.release.release_id,
            parsed.data.id,
            parsed.data.release_id,
            parsed.data.release.id,
            parsed.data.release.release_id,
        ];
        match ids.iter().copied().find(|id| *id != 0) {
            Some(id) => Ok(id),
            None => Err(format!(
                "auto_deploy response had no release id (body: {})",
                super::util::first_chars(&String::from_utf8_lossy(&raw), 200)
            )),
        }
    }

    /// Advances a PENDING release (the UI's "开始发布" click).
    pub fn rollout(&self, release_id: i64) -> Result<(), String> {
        let body = serde_json::json!({});
        self.request(
            "POST",
            &format!("/releases/{release_id}/rollout"),
            Some(&body),
        )?;
        Ok(())
    }

    /// The status of one release on a group, or `""` when the group's list
    /// does not mention it yet (the list is eventually consistent).
    pub fn release_status(&self, group_id: i64, release_id: i64) -> Result<String, String> {
        let releases = self.releases(group_id)?;
        Ok(releases
            .into_iter()
            .find(|release| release.id == release_id)
            .map(|release| release.status)
            .unwrap_or_default())
    }

    /// Polls until the release reaches a terminal state, auto-rolling out a
    /// PENDING one.
    ///
    /// The wait keys on the id `auto_deploy` returned, never on the image:
    /// re-publishing an unchanged image creates a second release with the
    /// same `Image URL` diff, and matching by image would report the older
    /// SUCCESS release while the new one stays PENDING.
    pub fn wait_release_by_id(
        &self,
        group_id: i64,
        release_id: i64,
        timeout_sec: i64,
        verbose: bool,
        step: Duration,
    ) -> Result<String, String> {
        let deadline = Instant::now() + Duration::from_secs(timeout_sec.max(0) as u64);
        let mut rollouts = 0;
        const MAX_ROLLOUTS: i32 = 3;
        loop {
            let status = self.release_status(group_id, release_id)?;
            match status.as_str() {
                "SUCCESS" => {
                    if verbose {
                        println!("  group={group_id} release={release_id} SUCCESS");
                    }
                    return Ok(status);
                }
                "REVERTED" | "REVOKED" | "FAILED" => {
                    return Err(format!("release {release_id} ended in {status}"));
                }
                "PENDING" if rollouts < MAX_ROLLOUTS => {
                    println!("  group={group_id} release={release_id} PENDING → rollout");
                    if let Err(error) = self.rollout(release_id) {
                        // A rollout that raced the state machine is reported
                        // as an error; the release may still be rolling.
                        println!(
                            "  group={group_id} release={release_id} rollout returned {error} (keeping polling)"
                        );
                    }
                    rollouts += 1;
                }
                _ => {}
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "release {release_id} on group {group_id} not SUCCESS after {timeout_sec}s \
                     (last status {status:?})"
                ));
            }
            if verbose {
                println!(
                    "  group={group_id} release={release_id} status={status:?} (retrying in {}s)",
                    step.as_secs()
                );
            }
            std::thread::sleep(step);
        }
    }
}

#[cfg(test)]
mod tests;
