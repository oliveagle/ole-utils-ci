// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The Captain response shapes, as far as `deploy-prod` needs them.

use serde::Deserialize;

/// The envelope the release list arrives in.
#[derive(Debug, Default, Deserialize)]
pub struct ReleasesEnvelope {
    #[serde(default)]
    pub result: Vec<super::CaptainRelease>,
}

/// The release id arrives under several shapes depending on the API version;
/// the first non-zero one wins.
#[derive(Debug, Default, Deserialize)]
pub struct AutoDeployResponse {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub release_id: i64,
    #[serde(default)]
    pub release: NestedRelease,
    #[serde(default)]
    pub data: NestedRelease,
}

/// One level of nesting around the release id.
#[derive(Debug, Default, Deserialize)]
pub struct NestedRelease {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub release_id: i64,
    #[serde(default)]
    pub release: InnerRelease,
}

/// The innermost release id holder.
#[derive(Debug, Default, Deserialize)]
pub struct InnerRelease {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub release_id: i64,
}
