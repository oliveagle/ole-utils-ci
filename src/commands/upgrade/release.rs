// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The GitLab release side of `upgrade`: which tag is newest, which release
//! asset matches this OS/arch, and how the binary is fetched.

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use serde::Deserialize;

use super::super::ci_recipe::join_install;
use super::super::ci_recipe_util::{go_errno, gunzip_stream_to_file, mkdir_all};
use crate::httpd;

use super::UpgradeConfig;

#[derive(Debug, Deserialize)]
pub(super) struct AssetLink {
    pub(super) name: String,
    pub(super) url: String,
}

/// The `assets` object of a release; absent or null behaves like empty.
#[derive(Debug, Deserialize)]
pub(super) struct Assets {
    #[serde(default)]
    pub(super) links: Vec<AssetLink>,
}

/// The fields of GitLab's `/releases/{tag}` response this command reads.
#[derive(Debug, Deserialize)]
struct GitlabRelease {
    #[serde(default)]
    assets: Option<Assets>,
}

/// One entry of GitLab's `/releases` list.
#[derive(Debug, Deserialize)]
struct ReleaseTag {
    #[serde(default)]
    tag_name: String,
}

/// One authenticated `GET`; an empty token sends no header at all.
fn get(token: &str, url: &str, timeout: Duration) -> Result<httpd::Response, String> {
    const NO_HEADERS: [(&str, &str); 0] = [];
    let authed: [(&str, &str); 1] = [("PRIVATE-TOKEN", token)];
    let headers = if token.is_empty() {
        NO_HEADERS.as_slice()
    } else {
        authed.as_slice()
    };
    httpd::request("GET", url, headers, None, timeout)
}

/// The first 512 bytes of a diagnostic body, trimmed the way Go's
/// `strings.TrimSpace` trims them.
fn peek(body: &[u8]) -> String {
    let head = &body[..body.len().min(512)];
    String::from_utf8_lossy(head).trim().to_string()
}

/// `fetchLatestVersion`: the first release tag that is not a release
/// candidate.
pub(super) fn fetch_latest_version(cfg: &UpgradeConfig) -> Result<String, String> {
    let url = format!(
        "{}/api/v4/projects/{}/releases",
        cfg.base_url.trim_end_matches('/'),
        cfg.project_id
    );
    let response = get(&cfg.token, &url, Duration::from_secs(30))?;
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "HTTP {}: {}",
            response.status,
            peek(&response.body)
        ));
    }
    let tags: Vec<ReleaseTag> = serde_json::from_slice(&response.body)
        .map_err(|error| format!("decode response: {error}"))?;
    tags.into_iter()
        .map(|tag| tag.tag_name)
        .find(|tag| !tag.contains("-rc."))
        .ok_or_else(|| "no stable release found".to_string())
}

/// `fetchReleaseInfo`: the asset list of one release tag.
pub(super) fn fetch_release_info(cfg: &UpgradeConfig) -> Result<Vec<AssetLink>, String> {
    let url = format!(
        "{}/api/v4/projects/{}/releases/{}",
        cfg.base_url.trim_end_matches('/'),
        cfg.project_id,
        cfg.version
    );
    let response = get(&cfg.token, &url, Duration::from_secs(30))?;
    if response.status == 404 {
        return Err(format!("release {} not found", cfg.version));
    }
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "HTTP {}: {}",
            response.status,
            peek(&response.body)
        ));
    }
    // `encoding/json` and `serde_json` spell a malformed body
    // differently; the `decode response: ` prefix and the exit code are
    // what the Go original promises.
    let release: GitlabRelease = serde_json::from_slice(&response.body)
        .map_err(|error| format!("decode response: {error}"))?;
    Ok(release
        .assets
        .map(|assets| assets.links)
        .unwrap_or_default())
}

/// `listAssetNames`: Go's `%v` of a `[]string` — `[a b c]`, never quoted.
pub(super) fn list_asset_names(links: &[AssetLink]) -> String {
    let names: Vec<&str> = links.iter().map(|link| link.name.as_str()).collect();
    format!("[{}]", names.join(" "))
}

/// `downloadAndInstall`: mkdir, fetch, gunzip, chmod. The directory is
/// created before the request, exactly like the Go original.
pub(super) fn download_and_install(
    download_url: &str,
    dir: &str,
    target_os: &str,
    target_arch: &str,
    token: &str,
) -> Result<(), String> {
    mkdir_all(dir).map_err(|error| format!("mkdir {dir}: {error}"))?;
    let response = get(token, download_url, Duration::from_secs(300))?;
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "HTTP {}: {}",
            response.status,
            response.status_display()
        ));
    }
    let output_name = format!("devine-utils-ci-{target_os}-{target_arch}");
    let output_path = join_install(dir, &output_name);
    gunzip_stream_to_file(&response.body, &output_path)?;
    std::fs::set_permissions(&output_path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("chmod: chmod {output_path}: {}", go_errno(&error)))?;
    Ok(())
}
