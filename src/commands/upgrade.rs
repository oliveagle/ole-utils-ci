// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `upgrade` command: install a release of this very tool from the
//! upstream GitLab releases.
//!
//! Rust port of the Go original's `internal/upgrade.go`. The command owns
//! a private `flag.FlagSet` and therefore receives the raw argument list,
//! exactly like the Go original — the shared global parser must never see
//! these flags. The positional version is taken *before* flag parsing
//! starts, so `upgrade v0.3.13 --arch arm64` works the way Go makes it
//! work.
//!
//! Token resolution is deliberately silent: a missing `dotvault` helper,
//! a failing helper or an empty key all fall through to `$GITLAB_TOKEN`,
//! because `lookupDotvaultToken`'s error is discarded on purpose.

use super::ci_recipe::{join_install, DEFAULT_BASE_URL, DEFAULT_PROJECT_ID};
use super::ci_recipe_util::verify_installed_binary;
use crate::cli::{report_parse_error, FlagSet};
use crate::gitlab;
use crate::static_regex;

/// The one tag shape a release may carry; `vX.Y.Z` and nothing else.
const VERSION_RE: &str = r"^v\d+\.\d+\.\d+$";

/// Everything one `upgrade` run needs after the flags are parsed.
#[derive(Debug, Clone)]
struct UpgradeConfig {
    /// Release tag, e.g. `v0.3.13`; `latest` resolves during `validate`.
    version: String,
    /// Install directory (Go default: `bin`).
    dir: String,
    /// Target architecture (`amd64` / `arm64`).
    arch: String,
    /// Target OS (`linux` / `darwin` / `windows`).
    os: String,
    /// Whether to run the post-install version check (Go default: true).
    verify: bool,
    /// GitLab base URL.
    base_url: String,
    /// GitLab project id.
    project_id: String,
    /// Resolved GitLab token, possibly empty.
    token: String,
}

/// The flag set `upgrade` registers, in Go's order and wording.
fn build_flags() -> FlagSet {
    let mut flags = FlagSet::new();
    flags.str_flag("dir", "bin");
    flags.str_flag("arch", "");
    flags.str_flag("os", "");
    flags.bool_flag("verify", true);
    flags.str_flag("base-url", DEFAULT_BASE_URL);
    flags.str_flag("project-id", DEFAULT_PROJECT_ID);
    flags.str_flag("token", "");
    flags.describe("dir", "install directory");
    flags.describe("arch", "target architecture (amd64, arm64)");
    flags.describe("os", "target OS (linux, darwin, windows)");
    flags.describe("verify", "verify installed binary version");
    flags.describe("base-url", "GitLab base URL");
    flags.describe("project-id", "GitLab project ID");
    flags.describe(
        "token",
        "GitLab token (default: dotvault RELEASE_BINARY_READ_TOKEN > $GITLAB_TOKEN)",
    );
    flags
}

/// One string flag's value; the flag set always holds the default.
fn text(flags: &FlagSet, name: &str) -> String {
    flags
        .get(name)
        .map(|value| value.as_str(""))
        .unwrap_or_default()
}

/// `parseUpgradeFlags`: the leading positional is the version, the rest
/// goes through the private flag set, then the token chain resolves.
fn parse_upgrade_flags(args: &[String]) -> Result<UpgradeConfig, String> {
    let mut version = String::new();
    let mut rest = args;
    if let Some((first, tail)) = args.split_first() {
        if !first.starts_with('-') {
            version = first.to_string();
            rest = tail;
        }
    }
    let mut flags = build_flags();
    if let Err(error) = flags.parse(rest) {
        report_parse_error(&error, &flags.render_usage("upgrade"));
        return Err(error.to_string());
    }
    let mut cfg = UpgradeConfig {
        version,
        dir: text(&flags, "dir"),
        arch: text(&flags, "arch"),
        os: text(&flags, "os"),
        verify: flags.get("verify").is_none_or(|value| value.as_bool(true)),
        base_url: text(&flags, "base-url"),
        project_id: text(&flags, "project-id"),
        token: text(&flags, "token"),
    };
    if cfg.token.is_empty() {
        // Go: `if v, err := lookupDotvaultToken(); err == nil && v != ""`.
        cfg.token = match gitlab::dotvault_get("RELEASE_BINARY_READ_TOKEN") {
            Ok(value) if !value.is_empty() => value,
            _ => std::env::var("GITLAB_TOKEN").unwrap_or_default(),
        };
    }
    Ok(cfg)
}

/// `runtime.GOOS` as Go spells it (`macos` is `darwin` there).
fn go_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    }
}

/// `runtime.GOARCH` as Go spells it (`x86_64` is `amd64` there).
fn go_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "x86" => "386",
        other => other,
    }
}

fn is_valid_arch(arch: &str) -> bool {
    arch == "amd64" || arch == "arm64"
}

fn is_valid_os(os: &str) -> bool {
    os == "linux" || os == "darwin" || os == "windows"
}

impl UpgradeConfig {
    /// Fills in the host's OS/architecture, resolves `latest`, then
    /// checks the tag and the two platform names — in Go's order, so the
    /// network is reached before the platform spellings are judged.
    fn validate(&mut self) -> Result<(), String> {
        if self.os.is_empty() {
            let detected = go_os();
            if !is_valid_os(detected) {
                return Err(format!("unsupported OS {detected:?}"));
            }
            self.os = detected.to_string();
        }
        if self.arch.is_empty() {
            let detected = go_arch();
            if !is_valid_arch(detected) {
                return Err(format!("unsupported architecture {detected:?}"));
            }
            self.arch = detected.to_string();
        }
        if self.version.is_empty() || self.version == "latest" {
            let latest = fetch_latest_version(self)
                .map_err(|error| format!("fetch latest version: {error}"))?;
            self.version = latest;
        }
        if !static_regex::compile(VERSION_RE).is_match(&self.version) {
            return Err(format!("version {:?} must match vX.Y.Z", self.version));
        }
        if !is_valid_arch(&self.arch) {
            return Err(format!(
                "unsupported arch {:?} (supported: amd64, arm64)",
                self.arch
            ));
        }
        if !is_valid_os(&self.os) {
            return Err(format!(
                "unsupported OS {:?} (supported: linux, darwin, windows)",
                self.os
            ));
        }
        Ok(())
    }
}

mod release;

use release::{download_and_install, fetch_latest_version, fetch_release_info, list_asset_names};

#[cfg(test)]
use release::AssetLink;

///
/// Returns the process exit code; every refusal travels up as an error so
/// `main` prints its own `Error:` line, exactly like Go's `runUpgrade`
/// calling `fail`.
pub fn upgrade(args: &[String]) -> Result<i32, String> {
    let mut cfg = parse_upgrade_flags(args)?;
    cfg.validate()?;
    run(cfg)
}

/// The install itself: look the release up, download the platform
/// artifact, then report and (optionally) check what landed.
fn run(cfg: UpgradeConfig) -> Result<i32, String> {
    let artifact_name = format!("devine-utils-ci-{}-{}.gz", cfg.os, cfg.arch);
    println!("🔍 Fetching release info for {}...", cfg.version);

    let links = fetch_release_info(&cfg).map_err(|error| format!("fetch release: {error}"))?;
    let Some(link) = links.iter().find(|link| link.name == artifact_name) else {
        return Err(format!(
            "artifact {:?} not found in release {}\nAvailable assets: {}",
            artifact_name,
            cfg.version,
            list_asset_names(&links)
        ));
    };

    println!("📦 Downloading {}...", link.name);
    download_and_install(&link.url, &cfg.dir, &cfg.os, &cfg.arch, &cfg.token)
        .map_err(|error| format!("download: {error}"))?;

    let output_name = format!("devine-utils-ci-{}-{}", cfg.os, cfg.arch);
    let installed_path = join_install(&cfg.dir, &output_name);
    println!("✅ Installed to {installed_path}");

    if cfg.verify {
        println!("🔍 Verifying version...");
        let expected = cfg.version.strip_prefix('v').unwrap_or(&cfg.version);
        verify_installed_binary(&installed_path, expected)
            .map_err(|error| format!("verify: {error}"))?;
        println!("✅ Version verified: {expected}");
    }

    Ok(0)
}

#[cfg(test)]
mod tests;
