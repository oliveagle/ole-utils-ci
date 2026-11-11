// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The three renderings of a [`Report`].
//!
//! The renderers live in their own module so the data model stays readable;
//! they are inherent methods, so the public API is unchanged.

use crate::output::{Mode, Report, Severity};

impl Report {
    /// Renders the report in `mode`.
    pub fn render(&self, mode: Mode) -> String {
        match mode {
            Mode::Text => self.render_text(),
            Mode::Json => self.render_json(),
            Mode::Toon => self.render_toon(),
        }
    }

    /// Human-readable rendering: summary, then one line per diagnostic.
    pub fn render_text(&self) -> String {
        let mut out = String::new();
        if !self.summary.is_empty() {
            out.push_str(&self.summary);
            out.push('\n');
        }
        for d in &self.diagnostics {
            let sev = match d.severity {
                Severity::Error => "ERROR",
                Severity::Warning => "WARN ",
                Severity::Info => "INFO ",
            };
            let mut loc = d.path.clone();
            if d.line > 0 {
                loc = format!("{}:{}", d.path, d.line);
                if d.col > 0 {
                    loc = format!("{loc}:{}", d.col);
                }
            }
            let rule = if d.rule.is_empty() {
                String::new()
            } else {
                format!("  {}:", d.rule)
            };
            out.push_str(&format!("  {sev}  {loc}{rule} {}\n", d.message));
            if !d.detail.is_empty() {
                out.push_str(&format!("         {}\n", d.detail));
            }
        }
        out
    }

    /// Canonical JSON rendering, terminated by a newline the way the Go
    /// original's `json.Encoder` does.
    pub fn render_json(&self) -> String {
        let mut text =
            serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text
    }

    /// TOON rendering. The payload is normalised through JSON first so both
    /// structured modes expose identical keys.
    pub fn render_toon(&self) -> String {
        let value = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        match toon_format::encode_default(&value) {
            Ok(text) => {
                if text.ends_with('\n') {
                    text
                } else {
                    format!("{text}\n")
                }
            }
            Err(e) => format!("# toon encode failed: {e}\n"),
        }
    }
}

/// Encodes an arbitrary payload the way the Go original's `writeOutput`
/// does for the commands that emit data rather than diagnostics.
///
/// * `--pretty` always wins and means `json` (the historical alias).
/// * the default (`""` or `toon`) is the compact TOON rendering;
/// * `json` is the pretty, two-space-indented canonical form;
/// * `text` is the legacy single-line JSON scripts stream.
pub fn encode_value(mode: &str, pretty: bool, value: &serde_json::Value) -> Result<String, String> {
    let selected = if pretty { "json" } else { mode };
    match selected.trim().to_ascii_lowercase().as_str() {
        "" | "toon" => {
            let text = toon_format::encode_default(value)
                .map_err(|e| format!("toon encode failed: {e}"))?;
            Ok(with_newline(text))
        }
        "json" => {
            let text = serde_json::to_string_pretty(value)
                .map_err(|e| format!("json encode failed: {e}"))?;
            Ok(with_newline(text))
        }
        "text" => {
            let text =
                serde_json::to_string(value).map_err(|e| format!("json encode failed: {e}"))?;
            Ok(with_newline(text))
        }
        // Go quotes the value exactly as the caller spelled it.
        _ => Err(format!(
            "unknown --output {mode:?}; use toon, json, or text"
        )),
    }
}

/// Appends the newline `json.Encoder` and the TOON encoder both produce.
fn with_newline(mut text: String) -> String {
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod encode_tests {
    use super::*;

    fn sample() -> serde_json::Value {
        serde_json::json!({"b": 2, "a": 1})
    }

    #[test]
    fn the_default_mode_is_toon() {
        let text = encode_value("", false, &sample()).unwrap();
        assert!(text.contains("b: 2"), "{text}");
        assert!(text.contains("a: 1"), "{text}");
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn pretty_overrides_everything_and_text_stays_single_line() {
        let pretty = encode_value("toon", true, &sample()).unwrap();
        assert!(pretty.contains("\n  \"a\": 1"), "{pretty}");
        let text = encode_value("text", false, &sample()).unwrap();
        assert_eq!(text, "{\"b\":2,\"a\":1}\n");
    }

    #[test]
    fn an_unknown_mode_quotes_the_original_spelling() {
        let err = encode_value(" YAML", false, &sample()).unwrap_err();
        assert_eq!(err, "unknown --output \" YAML\"; use toon, json, or text");
    }
}
