// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Everything `logs` does between the token check and the printed entries:
//! the OSG request body, the HTTP exchange, the three XML shapes OSG is known
//! to answer with, and the two renderings.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Local, NaiveDateTime};

use crate::{goparse, httpd, xml};

use super::LogsConfig;

/// Go puts a 30s client timeout around the query.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// `parseFromFlag`'s fallback window when `--from` is not a duration.
const TEN_MINUTES_NANOS: i64 = 600_000_000_000;

/// `time.Now` in the local zone. Every deadline is absolute time, so a fixed
/// offset is all the body formatting and the comparisons need.
pub(super) fn now() -> DateTime<FixedOffset> {
    Local::now().fixed_offset()
}

/// `parseFromFlag`: a duration relative to now; a value that is not a
/// duration silently means "the last ten minutes".
pub(super) fn parse_from_flag(raw: &str) -> DateTime<FixedOffset> {
    match goparse::parse_duration(raw) {
        Some(nanos) => shift(now(), nanos.wrapping_neg()),
        None => shift(now(), -TEN_MINUTES_NANOS),
    }
}

/// `parseTime`: three layouts in turn. A layout without a zone reads as UTC
/// (that is what `time.Parse` does), and anything unrecognised means "now".
pub(super) fn parse_time(raw: &str) -> DateTime<FixedOffset> {
    for layout in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(value) = NaiveDateTime::parse_from_str(raw, layout) {
            return value.and_utc().fixed_offset();
        }
    }
    if let Ok(value) = DateTime::parse_from_rfc3339(raw) {
        return value;
    }
    now()
}

/// Moves `base` by `nanos`, keeping its own wall clock.
fn shift(base: DateTime<FixedOffset>, nanos: i64) -> DateTime<FixedOffset> {
    base.checked_add_signed(chrono::Duration::nanoseconds(nanos))
        .unwrap_or(base)
}

/// `2006-01-02 15:04:05` — the OSG wire format. Rendered in the value's own
/// zone, which is what Go's `Format` does.
fn stamp(value: &DateTime<FixedOffset>) -> String {
    value.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// `buildLogsRequestBody`.
///
/// `encoding/json` sorts map keys, so both objects are built as `BTreeMap`s
/// and reach the server with the keys in the same order Go produces. Go's
/// `json.Marshal` also HTML-escapes string bodies, which
/// [`crate::gojson::to_go_json`] reproduces.
pub(super) fn build_request_body(
    cfg: &LogsConfig,
    from: &DateTime<FixedOffset>,
    to: &DateTime<FixedOffset>,
) -> Vec<u8> {
    let mut body: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    body.insert("appId".to_string(), serde_json::json!(cfg.app_id));
    body.insert("fromDate".to_string(), serde_json::json!(stamp(from)));
    body.insert("limit".to_string(), serde_json::json!(cfg.limit));
    body.insert("logLevel".to_string(), serde_json::json!(cfg.level));
    body.insert("queryLargeFields".to_string(), serde_json::json!(true));
    if !cfg.title.is_empty() {
        body.insert("title".to_string(), serde_json::json!(cfg.title));
    }
    body.insert("toDate".to_string(), serde_json::json!(stamp(to)));

    let mut payload: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    payload.insert("access_token".to_string(), serde_json::json!(cfg.token));
    payload.insert(
        "request_body".to_string(),
        serde_json::to_value(&body).unwrap_or_default(),
    );
    crate::gojson::to_go_json(&serde_json::to_string(&payload).unwrap_or_default()).into_bytes()
}

/// `logsQuery`: one request, then the entries.
pub(super) fn logs_query(
    cfg: &LogsConfig,
    from: DateTime<FixedOffset>,
    to: DateTime<FixedOffset>,
) -> Result<(), String> {
    let body = build_request_body(cfg, &from, &to);
    let headers = [
        ("Content-Type", "application/json"),
        ("X-Auth-Token", cfg.token.as_str()),
    ];
    let response = httpd::request("POST", &cfg.url, &headers, Some(&body), REQUEST_TIMEOUT)
        .map_err(|error| format!("http post: {error}"))?;
    if response.status >= 400 {
        return Err(format!(
            "OSG HTTP {}: {}",
            response.status,
            response.body_string().trim()
        ));
    }
    let entries = parse_clog_xml(&response.body);
    for entry in &entries {
        if cfg.output == "json" {
            println!("{}", entry.to_json());
        } else {
            println!("{}", entry.render());
        }
    }
    Ok(())
}

/// One parsed clog entry. The field order is the Go struct's, because JSON
/// mode prints the fields in declaration order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ClogEntry {
    pub(super) app_id: String,
    pub(super) title: String,
    pub(super) log_time: String,
    pub(super) log_level: String,
    pub(super) content: String,
    pub(super) message_id: String,
    pub(super) service_code: String,
    pub(super) operation: String,
}

impl ClogEntry {
    /// `json.Marshal(entry)`: declaration order, `omitempty` on the last
    /// three fields, and Go's HTML escaping.
    pub(super) fn to_json(&self) -> String {
        let mut map = serde_json::Map::new();
        map.insert("appId".to_string(), serde_json::json!(self.app_id));
        map.insert("title".to_string(), serde_json::json!(self.title));
        map.insert("logTime".to_string(), serde_json::json!(self.log_time));
        map.insert("logLevel".to_string(), serde_json::json!(self.log_level));
        map.insert("content".to_string(), serde_json::json!(self.content));
        for (name, value) in [
            ("messageId", &self.message_id),
            ("serviceCode", &self.service_code),
            ("operationName", &self.operation),
        ] {
            if !value.is_empty() {
                map.insert(name.to_string(), serde_json::json!(value));
            }
        }
        crate::gojson::to_go_json(&serde_json::to_string(&map).unwrap_or_default())
    }

    /// `ClogEntry.String()`: `logTime[:19] [level] title  content`, with TWO
    /// spaces before the content.
    pub(super) fn render(&self) -> String {
        format!(
            "{} [{}] {}  {}",
            first_19(&self.log_time),
            self.log_level,
            self.title,
            self.content
        )
    }
}

/// Go slices the string at byte 19; this stops at a character boundary so a
/// multi-byte `logTime` cannot panic.
fn first_19(value: &str) -> &str {
    if value.len() <= 19 {
        return value;
    }
    let mut end = 19;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

/// `parseClogXML`. Every failure means "no entries" — the Go original never
/// reports an error here, so a malformed body prints nothing and exits 0.
pub(super) fn parse_clog_xml(raw: &[u8]) -> Vec<ClogEntry> {
    let text = String::from_utf8_lossy(raw);
    let Ok(root) = xml::parse(&text) else {
        return Vec::new();
    };
    match root.name.as_str() {
        // <response><logs><log>...</log></logs></response>
        "response" => root
            .children_named("logs")
            .iter()
            .flat_map(|logs| logs.children_named("log"))
            .map(entry_from)
            .collect(),
        // <logs><log>...</log></logs>
        "logs" => root
            .children_named("log")
            .into_iter()
            .map(entry_from)
            .collect(),
        // <log><Entries>...</Entries></log>
        "log" => root
            .children_named("Entries")
            .into_iter()
            .map(entry_from)
            .collect(),
        _ => Vec::new(),
    }
}

/// One `<log>` element's fields, read by the Go struct's tags.
fn entry_from(node: &xml::Node) -> ClogEntry {
    ClogEntry {
        app_id: node.child_text("appId"),
        title: node.child_text("title"),
        log_time: node.child_text("logTime"),
        log_level: node.child_text("logLevel"),
        content: node.child_text("content"),
        message_id: node.child_text("messageId"),
        service_code: node.child_text("serviceCode"),
        operation: node.child_text("operationName"),
    }
}
