// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `ci-recipe` model: constants, validation, download URL and setup
//! script — the contract downstream repositories consume.
//!
//! Rust port of the Go original's `internal/ci_recipe.go`. The recipe is
//! the single source of truth for "download devine-utils-ci from a GitLab
//! release-job artifact": the URL a downstream `setup:` job curls, the
//! script it runs and the artifact names the release stage produces.
//! Changing any of it breaks every downstream pipeline, so wording and
//! ordering follow the Go original byte for byte.

use crate::static_regex;

/// The GitLab base URL of the upstream instance.
///
/// `http://` is intentional: the internal CA is not trusted on many
/// runners (devine_infra commit 6456d76).
pub const DEFAULT_BASE_URL: &str = "http://git.dev.sh.ctripcorp.com";

/// The upstream project id of `devine/devine_utils`.
pub const DEFAULT_PROJECT_ID: &str = "158442";

/// The release job in the upstream `.gitlab-ci.yml`.
pub const DEFAULT_RELEASE_JOB: &str = "release";

/// The default artifact: Linux amd64 covers every GitLab shared runner.
pub const DEFAULT_ARTIFACT: &str = "devine-utils-ci-linux-amd64.gz";

/// The exact filenames the upstream release stage produces. Downstream
/// repos must pick one of these; anything else is a typo.
pub const SUPPORTED_ARTIFACTS: [&str; 5] = [
    "devine-utils-ci-linux-amd64.gz",
    "devine-utils-ci-linux-arm64.gz",
    "devine-utils-ci-darwin-amd64.gz",
    "devine-utils-ci-darwin-arm64.gz",
    "devine-utils-ci-windows-amd64.exe.gz",
];

/// The knobs a downstream `.gitlab-ci.yml` has to configure to fetch the
/// binary from the upstream release job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CIRecipe {
    /// GitLab instance, e.g. `http://git.dev.sh.ctripcorp.com`.
    pub base_url: String,
    /// Upstream project id, e.g. `158442`.
    pub project_id: String,
    /// Upstream release tag, e.g. `v0.2.6`.
    pub version: String,
    /// Upstream release job name, e.g. `release`.
    pub release_job: String,
    /// Artifact filename from [`SUPPORTED_ARTIFACTS`].
    pub artifact: String,
    /// GitLab `JOB-TOKEN`; only `download` uses it.
    pub job_token: String,
    /// Where `download` puts the binary; the script defaults it to `bin`.
    pub install_dir: String,
    /// Whether `download` runs the post-install version check.
    pub expect_verify: bool,
}

impl Default for CIRecipe {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            project_id: DEFAULT_PROJECT_ID.to_string(),
            version: String::new(),
            release_job: DEFAULT_RELEASE_JOB.to_string(),
            artifact: DEFAULT_ARTIFACT.to_string(),
            job_token: String::new(),
            install_dir: "bin".to_string(),
            expect_verify: true,
        }
    }
}

impl CIRecipe {
    /// Checks the recipe is complete enough to produce a download URL,
    /// reporting the first problem the way the Go original words it.
    pub fn validate(&self) -> Result<(), String> {
        if self.base_url.is_empty() {
            return Err("ci-recipe: BaseURL is empty".to_string());
        }
        if self.project_id.is_empty() {
            return Err("ci-recipe: ProjectID is empty".to_string());
        }
        if self.version.is_empty() {
            return Err("ci-recipe: Version is empty (e.g. v0.2.6)".to_string());
        }
        if !static_regex::compile(r"^v\d+\.\d+\.\d+$").is_match(&self.version) {
            return Err(format!(
                "ci-recipe: Version {:?} must match vX.Y.Z",
                self.version
            ));
        }
        if self.release_job.is_empty() {
            return Err("ci-recipe: ReleaseJob is empty".to_string());
        }
        if self.artifact.is_empty() {
            return Err("ci-recipe: Artifact is empty".to_string());
        }
        if !SUPPORTED_ARTIFACTS.contains(&self.artifact.as_str()) {
            return Err(format!(
                "ci-recipe: Artifact {:?} is not one of [{}]",
                self.artifact,
                SUPPORTED_ARTIFACTS.join(" ")
            ));
        }
        Ok(())
    }

    /// The ref-based GitLab artifact URL: lock to the tag, never to a
    /// job id, so a new upstream release does not invalidate downstream
    /// recipes.
    pub fn artifact_url(&self) -> Result<String, String> {
        self.validate()?;
        let base = self.base_url.trim_end_matches('/');
        Ok(format!(
            "{base}/api/v4/projects/{}/jobs/artifacts/{}/raw/{}?job={}",
            self.project_id,
            self.version,
            artifact_path(&self.artifact),
            query_escape(&self.release_job)
        ))
    }

    /// The shell snippet a downstream `setup:` job runs — the runtime
    /// twin of this text is [`super::ci_recipe_util::download_ci_recipe_binary`].
    pub fn setup_script(&self) -> Result<String, String> {
        self.validate()?;
        let install_dir = if self.install_dir.is_empty() {
            "bin"
        } else {
            self.install_dir.as_str()
        };
        let bin_path = join_install(install_dir, "devine-utils-ci");
        let url = self.artifact_url()?;
        // `strip_prefix` drops the leading "v": v0.2.6 -> 0.2.6.
        let verify_block = if self.expect_verify {
            let expected = self.version.strip_prefix('v').unwrap_or(&self.version);
            format!(
                "\nEXPECTED=\"{expected}\"\nACTUAL=$(./{bin_path} version | awk '{{print $NF}}')\n\
                 test \"$ACTUAL\" = \"$EXPECTED\" || {{ echo \"version mismatch: \
                 want=$EXPECTED got=$ACTUAL\" >&2; exit 1; }}"
            )
        } else {
            String::new()
        };
        Ok(format!(
            "set -eu\n\
             mkdir -p {install_dir}\n\
             curl -fL --header \"JOB-TOKEN: ${{CI_JOB_TOKEN}}\" \\\n\
             \x20 \"{url}\" \\\n\
             \x20 -o /tmp/devine-utils-ci.gz\n\
             gunzip -f /tmp/devine-utils-ci.gz\n\
             mv /tmp/devine-utils-ci {bin_path}\n\
             chmod +x {bin_path}{verify_block}\n"
        ))
    }
}

/// The artifact's path inside the release-job zip.
fn artifact_path(artifact: &str) -> String {
    format!("dist/{artifact}")
}

/// `url.QueryEscape`: unreserved characters pass through, space becomes
/// `+`, every other byte is `%XX` in upper case.
fn query_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                escaped.push(byte as char);
            }
            b' ' => escaped.push('+'),
            other => escaped.push_str(&format!("%{other:02X}")),
        }
    }
    escaped
}

/// `path/filepath.Clean` for slash-separated paths: collapse duplicate
/// separators and `.`, resolve `..` without escaping the root.
pub(crate) fn go_clean(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." if absolute => {
                // `/..` is the root; popping an empty list is a no-op.
                let _ = parts.pop();
            }
            ".." => match parts.last() {
                Some(last) if *last != ".." => {
                    let _ = parts.pop();
                }
                _ => parts.push(".."),
            },
            other => parts.push(other),
        }
    }
    if absolute {
        format!("/{}", parts.join("/"))
    } else if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// `path/filepath.Dir`: the parent of the cleaned path (`"a"` -> `"."`,
/// `"/a"` -> `"/"`).
pub(crate) fn go_dir(path: &str) -> String {
    let cleaned = go_clean(path);
    match cleaned.rfind('/') {
        None => ".".to_string(),
        Some(0) => "/".to_string(),
        Some(index) => cleaned[..index].to_string(),
    }
}

/// `path/filepath.Join(dir, name)`: join then clean.
pub(crate) fn join_install(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        return go_clean(name);
    }
    go_clean(&format!("{dir}/{name}"))
}

#[cfg(test)]
mod tests;
