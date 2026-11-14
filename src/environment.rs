// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Where this run happens, and what that allows.
//!
//! The tool has two environments: `dev` is the developer's loop and `ci` is
//! the judge. A CI job is always the judge, so inside CI the tool refuses the
//! ways a run could be made weaker:
//!
//! * `--mode dev` is refused unless the pipeline declared it on purpose;
//! * `DEVINE_UTILS_RUST_DRIVER=blaze` is refused in `ci` mode: a fast driver
//!   answers from its own cache and can leave a test or a lint out.
//!
//! A CI pipeline states the mode it runs in the environment
//! (`DEVINE_UTILS_REQUIRE_MODE=ci`), and the tool enforces that statement too:
//!
//! * a job that pinned `ci` cannot be talked into running `dev`;
//! * a job that pinned nothing cannot fall back to `dev` either — it is
//!   refused, because a CI job that does not say what it runs is exactly the
//!   job that can be degraded by accident;
//! * only a job that pinned `dev` on purpose may run the developer's mode,
//!   and it says so in its log.

use crate::config::Mode;

/// The variable a CI pipeline sets to pin the mode it runs in.
pub const REQUIRE_MODE_VAR: &str = "DEVINE_UTILS_REQUIRE_MODE";

/// The variable that names the build driver.
pub const DRIVER_VAR: &str = "DEVINE_UTILS_RUST_DRIVER";

/// The variables a CI system sets.
const CI_VARS: [&str; 4] = ["CI", "GITHUB_ACTIONS", "GITLAB_CI", "BUILDKITE"];

/// Whether this run happens inside a CI system.
pub fn in_ci() -> bool {
    CI_VARS
        .iter()
        .any(|name| is_set(std::env::var(name).ok().as_deref()))
}

/// The mode the environment pins, if it pins one.
pub fn required_mode() -> Result<Option<Mode>, String> {
    match std::env::var(REQUIRE_MODE_VAR) {
        Ok(value) => parse_required_mode(&value),
        Err(_) => Ok(None),
    }
}

/// The build driver the environment asks for, if it asks for one.
pub fn requested_driver() -> Result<Option<String>, String> {
    match std::env::var(DRIVER_VAR) {
        Ok(value) => parse_driver(&value),
        Err(_) => Ok(None),
    }
}

/// Checks that this run may happen in `mode`.
///
/// `asked` says whether the command line named the mode. A job that pins a
/// mode and a command line that names another one contradict each other, and
/// that is an error; a command line that names no mode at all is not a
/// contradiction — the run then keeps the full gate, which is the safe side.
pub fn check(mode: Mode, asked: bool) -> Result<(), String> {
    check_with(mode, required_mode()?, in_ci(), requested_driver()?, asked)
}

/// One line describing where this run happens, for the run's output.
pub fn describe(mode: Mode) -> String {
    if in_ci() {
        format!(
            "{} environment (CI detected — full gate, cargo only)",
            mode.as_str()
        )
    } else {
        format!("{} environment", mode.as_str())
    }
}

/// Whether a variable counts as set.
fn is_set(value: Option<&str>) -> bool {
    match value {
        None => false,
        Some(value) => {
            let value = value.trim().to_ascii_lowercase();
            !value.is_empty() && value != "0" && value != "false"
        }
    }
}

/// Reads a `DEVINE_UTILS_REQUIRE_MODE` value.
fn parse_required_mode(value: &str) -> Result<Option<Mode>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    match value.to_ascii_lowercase().as_str() {
        "dev" => Ok(Some(Mode::Dev)),
        "ci" | "release" => Ok(Some(Mode::Ci)),
        other => Err(format!(
            "{REQUIRE_MODE_VAR} must be `ci` or `dev`, not `{other}`"
        )),
    }
}

/// Reads a `DEVINE_UTILS_RUST_DRIVER` value.
fn parse_driver(value: &str) -> Result<Option<String>, String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Ok(None);
    }
    match value.as_str() {
        "cargo" | "blaze" => Ok(Some(value)),
        other => Err(format!(
            "{DRIVER_VAR} must be `cargo` or `blaze`, not `{other}`"
        )),
    }
}

/// The rules, without the environment around them.
fn check_with(
    mode: Mode,
    required: Option<Mode>,
    ci: bool,
    driver: Option<String>,
    asked: bool,
) -> Result<(), String> {
    if asked {
        if let Some(required) = required {
            if required != mode {
                return Err(format!(
                    "this environment requires --mode {}, but the run asked for {}",
                    required.as_str(),
                    mode.as_str()
                ));
            }
        }
    }
    if !ci {
        return Ok(());
    }
    if mode.is_dev() && !(asked && required == Some(Mode::Dev)) {
        return Err(format!(
            "CI runs the authoritative gate: --mode dev is refused here \
             (it narrows the scope and leaves the slow gates out). \
             A pipeline that means to run it must say so: \
             {REQUIRE_MODE_VAR}=dev"
        ));
    }
    if driver.as_deref() == Some("blaze") {
        return Err(format!(
            "CI answers with cargo: {DRIVER_VAR}=blaze is refused in ci mode \
             (a fast driver answers from its own cache and can leave a test out)"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
