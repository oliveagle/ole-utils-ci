// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The JSON on both sides of the wire for `pipeline-run`.
//!
//! Go keeps the request body, the decoded `POST /pipelines` response and
//! the payload written to stdout next to the command in one file; the
//! file-length budget splits them out here instead. The field order of
//! [`PipelineRunResult`] is the Go struct's declaration order, because
//! `serde_json` is built with `preserve_order`.

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

use crate::httpd;
use crate::output::render::encode_value;

/// The zero value of Go's `time.Time`.
///
/// `created_at` is `omitempty` on a struct, and Go's `isEmptyValue` never
/// calls a struct empty, so this value always reaches the output.
pub(super) const ZERO_TIME: &str = "0001-01-01T00:00:00Z";

/// The JSON payload `pipeline-run` writes to stdout.
#[derive(Debug, Serialize)]
pub(super) struct PipelineRunResult {
    pub(super) project: String,
    #[serde(rename = "ref")]
    pub(super) reference: String,
    pub(super) status: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) message: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub(super) pipeline_id: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) pipeline_status: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) pipeline_web_url: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) pipeline_sha: String,
    pub(super) created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) waited_seconds: Option<f64>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) final_status: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) variables: BTreeMap<String, String>,
    meta: PipelineRunMeta,
}

/// `meta`: where the trigger went and when.
#[derive(Debug, Serialize)]
struct PipelineRunMeta {
    base_url: String,
    token_source: String,
    triggered_at: String,
}

impl PipelineRunResult {
    /// The result before GitLab has answered: everything except the
    /// pipeline fields and the wait outcome.
    pub(super) fn new(
        project: &str,
        reference: &str,
        variables: BTreeMap<String, String>,
        base_url: &str,
        token_source: &str,
    ) -> Self {
        PipelineRunResult {
            project: project.to_string(),
            reference: reference.to_string(),
            status: String::new(),
            message: String::new(),
            pipeline_id: 0,
            pipeline_status: String::new(),
            pipeline_web_url: String::new(),
            pipeline_sha: String::new(),
            created_at: ZERO_TIME.to_string(),
            waited_seconds: None,
            final_status: String::new(),
            variables,
            meta: PipelineRunMeta {
                base_url: base_url.to_string(),
                token_source: token_source.to_string(),
                triggered_at: go_rfc3339_now(),
            },
        }
    }

    /// The trigger answered with an HTTP error: report it, and keep the
    /// exit code at 0 the way the Go original does.
    pub(super) fn failed(&mut self, response: &httpd::Response) {
        self.status = "failed".to_string();
        self.message = format!(
            "GitLab API returned {}: {}",
            response.status_display(),
            truncate(&response.body_string(), 500)
        );
    }

    /// Fills the pipeline fields from a decoded trigger response.
    pub(super) fn created(&mut self, pipeline: PipelineRunResponse) {
        let id = pipeline.id;
        self.status = "created".to_string();
        self.pipeline_id = id;
        self.pipeline_status = pipeline.status;
        self.pipeline_web_url = pipeline.web_url;
        self.pipeline_sha = pipeline.sha;
        self.created_at = pipeline
            .created_at
            .map_or_else(|| ZERO_TIME.to_string(), go_rfc3339_nano);
        let message = format!("Pipeline #{id} created on {}", self.reference);
        self.message = message;
    }

    /// Folds a finished `--wait` poll into the result.
    pub(super) fn finished(&mut self, final_status: &str, elapsed: f64) {
        let id = self.pipeline_id;
        self.waited_seconds = Some(elapsed);
        self.final_status = final_status.to_string();
        self.pipeline_status = final_status.to_string();
        self.message = if final_status == "success" {
            format!("Pipeline #{id} completed successfully in {elapsed:.0}s")
        } else {
            format!("Pipeline #{id} finished with status {final_status:?} in {elapsed:.0}s")
        };
    }
}

/// The fields of GitLab's `POST /pipelines` response this command reads.
#[derive(Debug, Deserialize)]
pub(super) struct PipelineRunResponse {
    #[serde(default)]
    pub(super) id: i64,
    #[serde(default)]
    pub(super) status: String,
    #[serde(default, rename = "web_url")]
    pub(super) web_url: String,
    #[serde(default)]
    pub(super) sha: String,
    #[serde(default, deserialize_with = "go_time")]
    pub(super) created_at: Option<DateTime<FixedOffset>>,
}

/// A `*time.Time` of the Go struct: absent or `null` stays `nil`, and any
/// other spelling must parse as RFC 3339 the way `encoding/json` insists.
fn go_time<'de, D>(deserializer: D) -> Result<Option<DateTime<FixedOffset>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Option<String> = Option::deserialize(deserializer)?;
    match raw {
        None => Ok(None),
        Some(text) => DateTime::parse_from_rfc3339(&text)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

/// Go's `omitempty` on an `int`: zero disappears.
fn is_zero(value: &i64) -> bool {
    *value == 0
}

/// `time.Time.Format(time.RFC3339Nano)`: the fractional part only when the
/// value has one, with Go's trailing zeros trimmed.
fn go_rfc3339_nano(moment: DateTime<FixedOffset>) -> String {
    let mut out = moment.format("%Y-%m-%dT%H:%M:%S").to_string();
    let nanos = moment.timestamp_subsec_nanos();
    if nanos != 0 {
        let digits = format!("{nanos:09}");
        out.push('.');
        out.push_str(digits.trim_end_matches('0'));
    }
    let offset = moment.offset().local_minus_utc();
    if offset == 0 {
        out.push('Z');
        return out;
    }
    let (sign, seconds) = if offset < 0 {
        ('-', -offset)
    } else {
        ('+', offset)
    };
    out.push(sign);
    out.push_str(&format!(
        "{:02}:{:02}",
        seconds / 3600,
        (seconds % 3600) / 60
    ));
    out
}

/// `time.Now().UTC().Format(time.RFC3339)`.
fn go_rfc3339_now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Go's `truncate`: the first `max` bytes plus a marker.
///
/// Go slices bytes and can cut a rune in half; this backs off to a
/// character boundary so the reported body stays valid UTF-8.
pub(super) fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...(truncated)", &text[..end])
}

/// `parsePipelineRunVariables`: `KEY=VALUE` splits on the first `=`, a
/// value without one becomes an empty string, and a repeated key wins.
pub(super) fn parse_variables(raw: &[String]) -> BTreeMap<String, String> {
    raw.iter()
        .map(|item| match item.split_once('=') {
            Some((key, value)) => (key.to_string(), value.to_string()),
            None => (item.clone(), String::new()),
        })
        .collect()
}

/// The request body. Go marshals a `map[string]any`, so the keys come out
/// sorted — `ref` before `variables` — and every variable object reads
/// `key` before `value` the same way.
pub(super) fn build_payload(reference: &str, variables: &BTreeMap<String, String>) -> Vec<u8> {
    let mut body = serde_json::Map::new();
    body.insert(
        "ref".to_string(),
        serde_json::Value::String(reference.to_string()),
    );
    if !variables.is_empty() {
        let list: Vec<serde_json::Value> = variables
            .iter()
            .map(|(key, value)| serde_json::json!({"key": key, "value": value}))
            .collect();
        body.insert("variables".to_string(), serde_json::Value::Array(list));
    }
    // `json.Marshal` of this shape cannot fail either.
    serde_json::to_vec(&serde_json::Value::Object(body)).unwrap_or_default()
}

/// Decodes the trigger response, quoting the body when it is malformed the
/// way Go wraps `json.Unmarshal`'s error.
pub(super) fn decode_pipeline(body: &[u8]) -> Result<PipelineRunResponse, String> {
    serde_json::from_slice(body).map_err(|error| {
        format!(
            "pipeline-run: decode response: {error} (body: {})",
            truncate(&String::from_utf8_lossy(body), 300)
        )
    })
}

/// Encodes the result through the shared output contract. `--pretty` always
/// wins and means `json`, exactly like Go's `writeOutput`.
pub(super) fn render_output(
    mode: &str,
    pretty: bool,
    result: &PipelineRunResult,
) -> Result<String, String> {
    let value = serde_json::to_value(result).map_err(|error| error.to_string())?;
    encode_value(mode, pretty, &value)
}

/// `writeOutput`: render, then print. The encoding error travels up without
/// a `pipeline-run: ` prefix, exactly like Go's `return writeOutput(...)`.
pub(super) fn write_output(
    mode: &str,
    pretty: bool,
    result: &PipelineRunResult,
) -> Result<i32, String> {
    let text = render_output(mode, pretty, result)?;
    print!("{text}");
    Ok(0)
}
