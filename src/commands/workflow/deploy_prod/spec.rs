// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The spec guard: after the deploy, PROD must still publish the tool.

use std::time::Duration;

use serde::Deserialize;

use crate::commands::workflow::util::first_chars;
use crate::httpd;

/// One MCP tool description, as `/spec` publishes it.
#[derive(Debug, Deserialize)]
struct ToolEntry {
    #[serde(default)]
    name: String,
}

/// Fetches `<base>/spec` and asserts the tool is still exposed there.
pub fn verify_prod_spec(base_url: &str, want_tool: &str) -> Result<(), String> {
    let url = format!("{base_url}/spec");
    let response = httpd::request("GET", &url, &[], None, Duration::from_secs(15))
        .map_err(|error| format!("GET {url}: {error}"))?;
    let body = &response.body[..response.body.len().min(4 << 20)];
    let tools: Vec<ToolEntry> = serde_json::from_slice(body).map_err(|error| {
        format!(
            "parse {url}: {error} (body: {})",
            first_chars(&response.body_string(), 200)
        )
    })?;
    if tools.iter().any(|tool| tool.name == want_tool) {
        println!("  PROD spec OK — {want_tool} exposed on {url}");
        return Ok(());
    }
    Err(format!(
        "{url} does not expose {want_tool:?} ({} tools)",
        tools.len()
    ))
}
