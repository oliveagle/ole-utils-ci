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
