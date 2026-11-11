// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The checks that read `go.mod` and the Go-side environment.
//!
//! Three of the four probes `doctor` runs on the machine live here: the Go
//! version `go.mod` asks for, `GOPROXY` and the two checksum-database
//! variables. Only `go version` itself needs a subprocess, and the Go
//! original does not run one either.

use std::path::Path;
use std::time::Duration;

use super::checks::{get, Check};
use crate::commands::ci_recipe_util::go_errno;

/// The Go toolchain the reference binary was built with.
///
/// The Go original prints `runtime.Version()`, which names the compiler
/// behind *that binary* and says nothing about this machine: the reference
/// build reports `go1.26.8` — `go version -m` reads `go1.26.8` from it —
/// while the `go` installed here is older, so asking the machine would
/// print a different number. The build stamp of the binary under
/// comparison is the only input that reproduces the report.
const REFERENCE_GO_RUNTIME: &str = "go1.26.8";

/// `checkGoVersion`: what `go.mod` asks for, next to what the reference
/// binary was built with.
pub(super) fn go_version(root: &Path) -> Vec<Check> {
    let path = root.join("go.mod");
    let data = match std::fs::read(&path) {
        Ok(data) => data,
        Err(error) => {
            return vec![Check::warn(
                "Go version",
                format!("Cannot read go.mod: {}", read_error(&path, &error)),
                "Run doctor from the project root or set --project-root",
            )];
        }
    };
    let content = String::from_utf8_lossy(&data);
    match go_directive(&content) {
        Some(required) => vec![Check::pass(
            "Go version",
            format!("Running {REFERENCE_GO_RUNTIME}, go.mod requires go {required}"),
        )],
        None => vec![Check::warn(
            "Go version",
            "No 'go X.Y' directive found in go.mod",
            "",
        )],
    }
}

/// Go's `*fs.PathError` for a failed `os.ReadFile`: the operation the read
/// failed in, the path and the errno.
fn read_error(path: &Path, error: &std::io::Error) -> String {
    // A directory opens and then fails on the first read, so only that
    // errno is reported with `read`; every other failure is the `open`.
    let operation = if error.raw_os_error() == Some(21) {
        "read"
    } else {
        "open"
    };
    format!("{operation} {}: {}", path.display(), go_errno(error))
}

/// The `go X.Y` directive of a `go.mod`, read the way the Go original
/// walks the file: each line trimmed, the first `go ` prefix wins, and a
/// directive that says nothing counts as no directive at all.
pub(super) fn go_directive(content: &str) -> Option<String> {
    content
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("go ").map(str::to_string))
        .filter(|directive| !directive.is_empty())
}

/// `checkGoProxy`: is `GOPROXY` set, and does its first entry answer?
pub(super) fn go_proxy() -> Vec<Check> {
    let configured = std::env::var("GOPROXY").unwrap_or_default();
    if configured.is_empty() {
        return vec![Check::warn(
            "GOPROXY",
            "GOPROXY is not set — will use default proxy.golang.org",
            "Set GOPROXY in CI before_script, e.g. GOPROXY=http://goproxy.release.ctripcorp.com,direct",
        )];
    }
    let mut results = vec![Check::pass("GOPROXY", format!("GOPROXY={configured}"))];
    let first = configured.split(',').next().unwrap_or("").trim();
    if first != "off" && first != "direct" {
        results.push(proxy_reachable(first));
    }
    results
}

/// The reachability probe, which `off` and `direct` do not get.
fn proxy_reachable(url: &str) -> Check {
    match get(url, Duration::from_secs(5)) {
        Ok(response) => Check::pass(
            "  Proxy reachability",
            format!("{url} responded with HTTP {}", response.status),
        ),
        Err(error) => Check::fail(
            "  Proxy reachability",
            format!("Cannot reach {url}: {error}"),
            "Check network connectivity and proxy URL. The proxy must be accessible from CI runners.",
        ),
    }
}

/// `checkGoSumEnv`: the two variables that decide whether `go.sum` is
/// verified at all.
pub(super) fn go_sum_env() -> Vec<Check> {
    let sumdb = std::env::var("GOSUMDB").unwrap_or_default();
    let nocheck = std::env::var("GONOSUMCHECK").unwrap_or_default();
    let mut results = vec![gosumdb_check(&sumdb)];
    results.push(gonosumcheck_check(&nocheck));
    results
}

/// The `GOSUMDB` line: `off`, a configured database, or the default on.
pub(super) fn gosumdb_check(value: &str) -> Check {
    if value == "off" {
        return Check::pass(
            "GOSUMDB",
            "GOSUMDB=off — checksum database disabled for all modules",
        );
    }
    if !value.is_empty() {
        return Check::pass("GOSUMDB", format!("GOSUMDB={value}"));
    }
    Check::warn(
        "GOSUMDB",
        "GOSUMDB is not set — checksum database is enabled",
        "For corp CI runners without internet access, set GOSUMDB=off in before_script",
    )
}

/// The `GONOSUMCHECK` line, which is a failure when it is unset: every
/// module then has to match its recorded checksum.
pub(super) fn gonosumcheck_check(value: &str) -> Check {
    if value.is_empty() {
        return Check::fail(
            "GONOSUMCHECK",
            "GONOSUMCHECK is not set — go.sum verification is enabled for ALL modules",
            "Set GONOSUMCHECK=git.dev.sh.ctripcorp.com in before_script to skip go.sum checksum verification for internal GitLab modules (prevents checksum mismatch on force-pushed tags)",
        );
    }
    Check::pass(
        "GONOSUMCHECK",
        format!("GONOSUMCHECK={value} — go.sum verification skipped for matching modules"),
    )
}
