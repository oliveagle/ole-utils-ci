// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The HTTP face of [`super::kms_stub`].
//!
//! Go routes both endpoints through `net/http` and marshals a
//! `map[string]any`, so the reply keys arrive sorted and a refused lookup
//! carries no `result` at all. This module reproduces that shape on top of
//! the hand-written server in [`crate::httpd`], because the gates cannot
//! audit a HTTP crate `blaze` may not compile.

use serde::Deserialize;

use crate::httpd::Request;

use super::kms_stub::KmsStubData;

/// One HTTP reply the stub can send.
#[derive(Debug, PartialEq, Eq)]
pub struct Reply {
    /// The status line, e.g. `200 OK`.
    pub status: &'static str,
    /// The body, already encoded.
    pub body: Vec<u8>,
    /// The `Content-Type` of the body.
    pub content_type: &'static str,
    /// Whether `X-Content-Type-Options: nosniff` belongs on the response.
    pub nosniff: bool,
}

/// The body Go's `http.Error` writes for `405`.
const METHOD_NOT_ALLOWED: &str = "method not allowed\n";

/// The body Go's `http.NotFound` writes.
const NOT_FOUND: &str = "404 page not found\n";

/// Builds the one JSON reply shape every endpoint shares.
///
/// Go marshals a `map[string]any`, so the keys arrive sorted: `code`,
/// `message`, `result`. `result` only exists on success and `message` only
/// when there is one.
fn json_reply(code: i64, message: &str, result: Option<serde_json::Value>) -> Reply {
    let mut body = serde_json::Map::new();
    body.insert("code".to_string(), code.into());
    if !message.is_empty() {
        body.insert("message".to_string(), message.into());
    }
    if code == 0 {
        body.insert(
            "result".to_string(),
            result.unwrap_or(serde_json::Value::Null),
        );
    }
    let mut encoded = serde_json::to_vec(&body).unwrap_or_default();
    encoded.push(b'\n');
    Reply {
        status: "200 OK",
        body: encoded,
        content_type: "application/json",
        nosniff: false,
    }
}

/// Builds a `text/plain` reply the way `net/http` does.
fn plain(status: &'static str, body: &str) -> Reply {
    Reply {
        status,
        body: body.as_bytes().to_vec(),
        content_type: "text/plain; charset=utf-8",
        nosniff: true,
    }
}

/// Serves one parsed request against the stub data.
pub fn respond(data: &KmsStubData, request: &Request) -> Reply {
    match request.path.as_str() {
        "/query-pwd" if request.method == "POST" => query_pwd(data, request),
        "/query-key" if request.method == "POST" => query_key(data, request),
        "/query-pwd" | "/query-key" => plain("405 Method Not Allowed", METHOD_NOT_ALLOWED),
        _ => plain("404 Not Found", NOT_FOUND),
    }
}

/// The fields `/query-pwd` reads from the request body.
#[derive(Debug, Default, Deserialize)]
struct PwdRequest {
    /// The app id the caller claims.
    #[serde(default, rename = "appid")]
    app_id: String,
    /// The token to look up.
    #[serde(default)]
    token: String,
    /// The requested encoding; empty or `JSON` are both accepted.
    #[serde(default, rename = "type")]
    kind: String,
}

/// The fields `/query-key` reads from the request body.
#[derive(Debug, Default, Deserialize)]
struct KeyRequest {
    /// The app id the caller claims.
    #[serde(default, rename = "appid")]
    app_id: String,
    /// The token to look up.
    #[serde(default)]
    token: String,
}

/// Decodes the first JSON value of `body`, ignoring anything after it the
/// way `json.Decoder` does, and maps a decode failure to `None`.
fn decode<T: serde::de::DeserializeOwned>(body: &[u8]) -> Option<T> {
    let mut stream = serde_json::Deserializer::from_slice(body).into_iter::<T>();
    stream.next().and_then(Result::ok)
}

/// Checks the app id and herald token, returning the reply code and message.
fn authorize(data: &KmsStubData, app_id: &str, request: &Request) -> (i64, &'static str) {
    let want = data.app_id.trim();
    if !want.is_empty() && app_id.trim() != want {
        return (1, "appid mismatch");
    }
    let want = data.herald_token.trim();
    if request.header("herald-token").unwrap_or("").trim() != want && !want.is_empty() {
        return (2, "herald-token mismatch");
    }
    (0, "")
}

/// Answers `POST /query-pwd`.
fn query_pwd(data: &KmsStubData, request: &Request) -> Reply {
    let Some(body) = decode::<PwdRequest>(&request.body) else {
        return json_reply(1, "bad request", None);
    };
    let (code, message) = authorize(data, &body.app_id, request);
    if code != 0 {
        return json_reply(code, message, None);
    }
    let Some(record) = data.pwd.get(body.token.trim()) else {
        return json_reply(3, "unknown pwd token", None);
    };
    if !body.kind.is_empty() && !body.kind.eq_ignore_ascii_case("JSON") {
        return json_reply(4, "unsupported type", None);
    }
    json_reply(
        0,
        "",
        Some(serde_json::json!({
            "pwdAccount": record.account,
            "pwdTitle": record.title,
            "pwdValue": record.value,
            "type": "PWD",
        })),
    )
}

/// Answers `POST /query-key`.
fn query_key(data: &KmsStubData, request: &Request) -> Reply {
    let Some(body) = decode::<KeyRequest>(&request.body) else {
        return json_reply(1, "bad request", None);
    };
    let (code, message) = authorize(data, &body.app_id, request);
    if code != 0 {
        return json_reply(code, message, None);
    }
    let Some(record) = data.key.get(body.token.trim()) else {
        return json_reply(3, "unknown key token", None);
    };
    json_reply(
        0,
        "",
        Some(serde_json::json!({
            "keyName": record.name,
            "keyType": record.kind,
            "keyValue": record.value,
            "keyValueJson": record.value_json,
            "type": "KEY",
        })),
    )
}
