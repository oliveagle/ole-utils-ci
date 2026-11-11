// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `release` command: create a GitLab release for the current tag.
//!
//! Token resolution order (same as the Go original):
//! `GITLAB_RELEASE_TOKEN` → `GITLAB_TOKEN` → `CI_JOB_TOKEN`, the last one
//! authenticated with `JOB-TOKEN` instead of `PRIVATE-TOKEN`.

use std::path::Path;
use std::time::Duration;

use crate::config::Config;
use crate::httpd;

/// The display name used to label the release.
///
/// Resolution order: `DEVINE_UTILS_CI_RELEASE_NAME` (explicit override, so
/// `devine_utils`' own release job can keep the historical product name),
/// then the real `CI_PROJECT_NAME`, then `devine-utils-ci`.
fn project_name(override_name: &str, ci_project_name: &str) -> String {
    if !override_name.is_empty() {
        return override_name.to_string();
    }
    if !ci_project_name.is_empty() {
        return ci_project_name.to_string();
    }
    "devine-utils-ci".to_string()
}

/// Builds the release description and the asset links from a `dist/` listing.
///
/// Returns `(description, links)`; with no artifacts the description is the
/// plain `"{name} {tag}"` line and there are no links.
fn describe(
    name: &str,
    tag: &str,
    api_url: &str,
    project_id: &str,
    files: &[String],
) -> (String, Vec<serde_json::Value>) {
    if files.is_empty() {
        return (format!("{name} {tag}"), Vec::new());
    }
    let mut description = format!("# {name} {tag}\n\n## Release Binaries\n\n");
    let mut links = Vec::new();
    for file in files {
        description.push_str(&format!("- {file}\n"));
        let url = format!(
            "{api_url}/projects/{project_id}/jobs/{}/artifacts/dist/{file}",
            std::env::var("CI_JOB_ID").unwrap_or_default()
        );
        links.push(serde_json::json!({"link_type": "other", "name": file, "url": url}));
    }
    (description, links)
}

/// The non-directory entries of `dist/`, sorted the way `os.ReadDir` sorts.
fn dist_files(root: &Path) -> Vec<String> {
    let dir = root.join("dist");
    if !dir.is_dir() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<String> = entries
        .flatten()
        .filter(|entry| !entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    files.sort();
    files
}

/// Creates the release through the GitLab Releases API.
pub fn release(cfg: &Config) -> Result<i32, String> {
    let tag = std::env::var("CI_COMMIT_TAG").unwrap_or_default();
    let api_url = std::env::var("CI_API_V4_URL").unwrap_or_default();
    let project_id = std::env::var("CI_PROJECT_ID").unwrap_or_default();
    if tag.is_empty() || api_url.is_empty() || project_id.is_empty() {
        return Err("release requires CI environment: CI_COMMIT_TAG, CI_API_V4_URL, CI_PROJECT_ID must all be set (run inside GitLab CI tag pipeline)".to_string());
    }

    let (token, auth_header) = [
        ("GITLAB_RELEASE_TOKEN", "PRIVATE-TOKEN"),
        ("GITLAB_TOKEN", "PRIVATE-TOKEN"),
        ("CI_JOB_TOKEN", "JOB-TOKEN"),
    ]
    .into_iter()
    .find_map(|(env, header)| {
        let value = std::env::var(env).unwrap_or_default();
        if value.is_empty() {
            None
        } else {
            Some((value, header))
        }
    })
    .ok_or_else(|| {
        "no API token found: set GITLAB_RELEASE_TOKEN (recommended), GITLAB_TOKEN, or ensure CI_JOB_TOKEN has api scope".to_string()
    })?;

    let name = project_name(
        &std::env::var("DEVINE_UTILS_CI_RELEASE_NAME").unwrap_or_default(),
        &std::env::var("CI_PROJECT_NAME").unwrap_or_default(),
    );
    let files = dist_files(cfg.root());
    let (description, links) = describe(&name, &tag, &api_url, &project_id, &files);

    // `encoding/json` sorts map keys, so the payload is assembled in
    // alphabetical order to stay byte-identical.
    let mut body = serde_json::Map::new();
    if !links.is_empty() {
        body.insert("assets".to_string(), serde_json::json!({"links": links}));
    }
    body.insert("description".to_string(), serde_json::json!(description));
    body.insert(
        "name".to_string(),
        serde_json::json!(format!("{name} {tag}")),
    );
    body.insert("tag_name".to_string(), serde_json::json!(tag));
    let payload = serde_json::to_string(&serde_json::Value::Object(body))
        .map_err(|e| format!("failed to marshal release body: {e}"))?;

    let url = format!("{api_url}/projects/{project_id}/releases");
    let response = httpd::request(
        "POST",
        &url,
        &[
            (auth_header, token.as_str()),
            ("Content-Type", "application/json"),
        ],
        Some(payload.as_bytes()),
        Duration::from_secs(30),
    )
    .map_err(|e| format!("release API call failed: {e}"))?;
    if response.status == 409 {
        // The release already exists — idempotent runs are not an error.
        return Ok(0);
    }
    if response.status >= 300 {
        return Err(format!(
            "release API {}: {}",
            response.status_display(),
            response.body_string()
        ));
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_name_prefers_the_override() {
        assert_eq!(
            project_name("devine-utils-ci", "devine_cli"),
            "devine-utils-ci"
        );
        assert_eq!(project_name("", "devine_cli"), "devine_cli");
        assert_eq!(project_name("", ""), "devine-utils-ci");
    }

    #[test]
    fn empty_dist_yields_the_plain_description() {
        let (description, links) = describe("devine-utils-ci", "v1.2.3", "https://x", "7", &[]);
        assert_eq!(description, "devine-utils-ci v1.2.3");
        assert!(links.is_empty());
    }

    #[test]
    fn dist_files_are_sorted_and_skip_directories() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("dist")).unwrap();
        std::fs::write(dir.path().join("dist/b.bin"), b"b").unwrap();
        std::fs::write(dir.path().join("dist/a.bin"), b"a").unwrap();
        std::fs::create_dir(dir.path().join("dist/sub")).unwrap();
        assert_eq!(dist_files(dir.path()), vec!["a.bin", "b.bin"]);
        assert_eq!(
            dist_files(dir.path().join("missing").as_path()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn payload_keys_are_alphabetical() {
        let body = serde_json::json!({
            "assets": {"links": [{"link_type": "other", "name": "a.bin", "url": "u"}]},
            "description": "d",
            "name": "n",
            "tag_name": "v1.0.0",
        });
        let encoded = serde_json::to_string(&body).unwrap();
        assert_eq!(
            encoded,
            r#"{"assets":{"links":[{"link_type":"other","name":"a.bin","url":"u"}]},"description":"d","name":"n","tag_name":"v1.0.0"}"#
        );
    }
}
