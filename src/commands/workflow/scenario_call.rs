// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! One MCP `tools/call` exchange, normalised into a scenario verdict.

use std::time::Duration;

use serde::Deserialize;

use crate::httpd;

use super::scenarios::ScenarioResult;

/// The JSON-RPC envelope a scenario tool answers with.
#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(default)]
    error: Option<EnvelopeError>,
    #[serde(default)]
    result: Option<EnvelopeResult>,
}

#[derive(Debug, Deserialize)]
struct EnvelopeError {
    #[serde(default)]
    message: String,
}

#[derive(Debug, Deserialize)]
struct EnvelopeResult {
    #[serde(default)]
    content: Vec<ContentItem>,
}

#[derive(Debug, Deserialize)]
struct ContentItem {
    #[serde(default)]
    text: String,
}

/// Calls one scenario tool over MCP and normalises the outcome.
pub fn run_scenario_tool(env: &str, app: &str, base: &str, tool: &str) -> ScenarioResult {
    let mut result = ScenarioResult {
        app: app.to_string(),
        env: env.to_string(),
        base: base.to_string(),
        tool: tool.to_string(),
        ..ScenarioResult::default()
    };
    let body = serde_json::json!({
        "id": 1,
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"arguments": {}, "name": tool},
    });
    let payload = serde_json::to_vec(&body).unwrap_or_default();
    let url = format!("{base}/mcp");
    let headers = [
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/event-stream"),
    ];
    let response = match httpd::request(
        "POST",
        &url,
        &headers,
        Some(&payload),
        Duration::from_secs(90),
    ) {
        Ok(response) => response,
        Err(error) => {
            result.verdict = "ERROR".to_string();
            result.detail = error;
            return result;
        }
    };
    if response.status != 200 {
        result.verdict = "ERROR".to_string();
        result.detail = format!("HTTP {}", response.status);
        return result;
    }
    let envelope = match serde_json::from_str::<Envelope>(&strip_sse(&response.body_string())) {
        Ok(envelope) => envelope,
        Err(_) => {
            result.verdict = "ERROR".to_string();
            result.detail = "unparseable JSON-RPC response".to_string();
            return result;
        }
    };
    if let Some(error) = envelope.error {
        result.verdict = if error.message.contains("not found") {
            "MISSING".to_string()
        } else {
            "ERROR".to_string()
        };
        result.detail = error.message;
        return result;
    }
    let Some(call_result) = envelope.result else {
        result.verdict = "ERROR".to_string();
        result.detail = "empty result".to_string();
        return result;
    };
    let Some(first) = call_result.content.first() else {
        result.verdict = "ERROR".to_string();
        result.detail = "empty result".to_string();
        return result;
    };
    let payload: serde_json::Value = match serde_json::from_str(&first.text) {
        Ok(payload) => payload,
        Err(_) => {
            result.verdict = "ERROR".to_string();
            result.detail = "unparseable tool payload".to_string();
            return result;
        }
    };
    result.verdict = match payload.get("verdict").and_then(|value| value.as_str()) {
        Some("PASS") => "PASS".to_string(),
        Some("FAIL") => "FAIL".to_string(),
        _ => "ERROR".to_string(),
    };
    apply_payload_details(&mut result, &payload);
    result
}

/// Copies the optional detail fields out of a tool's payload.
///
/// The Go original walks a map, so `detail` and `failedCheck` both write the
/// detail field and the last one visited wins; the order is fixed here so the
/// output is reproducible.
fn apply_payload_details(result: &mut ScenarioResult, payload: &serde_json::Value) {
    for (key, target) in [
        ("environment", &mut result.environment),
        ("selfIdc", &mut result.self_idc),
        ("peerIdc", &mut result.peer_idc),
    ] {
        // Go only fills these while `detail` is still empty, and never writes
        // them back into it: `environment` alone must not mask `selfIdc`.
        if result.detail.is_empty() {
            if let Some(value) = payload.get(key).and_then(|value| value.as_str()) {
                *target = value.to_string();
            }
        }
    }
    for key in ["detail", "failedCheck"] {
        if let Some(value) = payload.get(key).and_then(|value| value.as_str()) {
            result.detail = value.to_string();
        }
    }
}

/// Strips the `data: ` prefix of a single-frame Streamable-HTTP response.
pub fn strip_sse(body: &str) -> String {
    let Some(index) = body.find("data: ") else {
        return body.to_string();
    };
    for line in body[index..].split('\n') {
        if let Some(rest) = line.trim().strip_prefix("data: ") {
            return rest.to_string();
        }
    }
    body.to_string()
}

#[cfg(test)]
mod tests;
