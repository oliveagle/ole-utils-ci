// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `install` subcommand: fetch the module dependencies.
//!
//! The Go original walks a list of proxies: the one the environment names,
//! the internal one, the public one and finally `direct`. A failure that
//! looks like a proxy or network problem moves on to the next proxy; any
//! other failure — a broken `go.mod`, a module nobody serves — is reported
//! at once, because no other proxy would do better.

use std::path::Path;
use std::time::Duration;

use crate::config::Config;
use crate::runner::Runner;

/// The proxy `go` uses when the environment names none.
const DEFAULT_GOPROXY: &str = "https://proxy.golang.org,direct";

/// The internal Go module proxy.
///
/// It serves the internal modules without a git login, which is what a CI
/// runner without git credentials needs.
const CORP_GOPROXY: &str = "http://goproxy.release.ctripcorp.com,http://artifactory.release.ctripcorp.com/artifactory/api/go/trip-golang-prod,direct";

/// The failures that mean "this proxy is broken, try the next one".
const PROXY_ERROR_PATTERNS: [&str; 10] = [
    "401",
    "403",
    "tls:",
    "certificate",
    "unknown authority",
    "connection refused",
    "no such host",
    "timeout",
    "eof",
    "terminal prompts disabled",
];

/// The wall-clock budget one proxy attempt gets.
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(300);

/// The `install` subcommand: fetch the module dependencies.
pub fn install(cfg: &Config) -> Result<i32, String> {
    install_dependencies(cfg).map(|()| 0)
}

/// Fetches the module dependencies, the way the Go original does.
///
/// A project that already vendors its dependencies is left alone; every
/// other project runs `go mod vendor` once per proxy.
pub fn install_dependencies(cfg: &Config) -> Result<(), String> {
    if vendor_exists(cfg.root()) {
        return Ok(());
    }
    let initial = match std::env::var("GOPROXY") {
        Ok(value) if !value.is_empty() => value,
        _ => CORP_GOPROXY.to_string(),
    };
    attempt(cfg, &initial)
        .map_err(|e| format!("dependency install failed after all proxy fallbacks: {e}"))
}

/// Whether the project already vendors its dependencies.
fn vendor_exists(root: &Path) -> bool {
    match std::fs::metadata(root.join("vendor")) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => true,
    }
}

/// Runs `go mod vendor` once per proxy until one of them works.
fn attempt(cfg: &Config, initial: &str) -> Result<(), String> {
    let runner = Runner::new(cfg.root(), cfg.verbose);
    let mut last_error = String::from("no proxy attempt made");
    for proxy in fallbacks(initial) {
        set_env(&proxy);
        let result = runner.run_go(ATTEMPT_TIMEOUT, "go", &["mod", "vendor"]);
        let Some(error) = result.error else {
            return Ok(());
        };
        last_error = error.clone();
        let message = format!("{error}\n{}", result.output);
        if is_proxy_error(&message) {
            continue;
        }
        return Err(format!("go mod vendor failed: {error}\n{}", result.output));
    }
    Err(format!(
        "all proxy attempts exhausted: {last_error}\n  Try checking your go.mod or network connectivity"
    ))
}

/// The proxies to try, in order and without repeats.
fn fallbacks(initial: &str) -> Vec<String> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for proxy in [initial, CORP_GOPROXY, DEFAULT_GOPROXY, "direct"] {
        if !seen.contains(&proxy) {
            seen.push(proxy);
            out.push(proxy.to_string());
        }
    }
    out
}

/// The three environment variables every attempt sets.
///
/// `GOPRIVATE` is deliberately left alone: it makes Go bypass the proxy and
/// clone from git, which is exactly what fails on a runner without git
/// credentials. `GONOSUMCHECK` covers the force-pushed internal tag instead.
fn set_env(proxy: &str) {
    std::env::set_var("GOPROXY", proxy);
    std::env::set_var("GOTOOLCHAIN", "local");
    std::env::set_var("GONOSUMCHECK", "git.dev.sh.ctripcorp.com");
}

/// Whether the failure looks like a proxy or network problem.
fn is_proxy_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    PROXY_ERROR_PATTERNS
        .iter()
        .any(|pattern| lower.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fallback_list_keeps_its_order_and_drops_repeats() {
        let list = fallbacks(CORP_GOPROXY);
        assert_eq!(list[0], CORP_GOPROXY);
        assert_eq!(list[1], DEFAULT_GOPROXY);
        assert_eq!(list[2], "direct");
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn a_network_failure_is_a_proxy_failure() {
        assert!(is_proxy_error("connection refused"));
        assert!(is_proxy_error(
            "x509: certificate signed by unknown authority"
        ));
        assert!(is_proxy_error("terminal prompts disabled"));
        assert!(!is_proxy_error("go.mod file not found"));
    }
}
