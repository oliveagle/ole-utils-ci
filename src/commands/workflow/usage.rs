// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! `printWorkflowUsage`: the help text for one subcommand, or the general
//! block when no subcommand matched.
//!
//! The callers choose the stream: help and `-h` go to stdout, a usage error
//! goes to stderr before the `Error:` line `main` adds.

use std::io::Write;

use super::model::DEFAULT_WORKFLOW_CONFIG_PATH;
use super::usage_general::GENERAL;
use super::usage_subs::{
    APPS, DEPLOY_PROD, INTEGRATION_RC, PROD_VERIFY, PROMOTE, RELEASE_RC, SCENARIOS, STATUS, TEST_RC,
};

/// The general block, with the user config path filled in.
pub fn general_text() -> String {
    GENERAL.replace("$CONFIG$", DEFAULT_WORKFLOW_CONFIG_PATH)
}

/// The block that belongs to `sub` (empty or unknown means general).
pub fn usage_text(sub: &str) -> String {
    let named = match sub {
        "prod-verify" => Some(PROD_VERIFY),
        "apps" => Some(APPS),
        "release-rc" => Some(RELEASE_RC),
        "test-rc" => Some(TEST_RC),
        "integration-rc" => Some(INTEGRATION_RC),
        "promote" => Some(PROMOTE),
        "deploy-prod" => Some(DEPLOY_PROD),
        "scenarios" => Some(SCENARIOS),
        "status" => Some(STATUS),
        _ => None,
    };
    match named {
        Some(text) => text.to_string(),
        None => general_text(),
    }
}

/// Writes the block for `sub` to `w`.
pub fn print_usage(w: &mut dyn Write, sub: &str) {
    let _ = w.write_all(usage_text(sub).as_bytes());
}

#[cfg(test)]
mod tests;
