// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The JUnit XML document.
//!
//! The document is the one Go's `encoding/xml` writes: the same attributes
//! in the same order, the same indentation and the same empty elements.

use super::Suite;

/// The JUnit XML document for the given suites.
///
/// The document is the one Go's `encoding/xml` writes: the same attributes
/// in the same order, the same indentation and the same empty elements.
pub fn render(suites: &[Suite]) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<testsuites tests=\"0\" failures=\"0\" skipped=\"0\" time=\"0\">");
    if suites.is_empty() {
        out.push_str("</testsuites>");
        return out;
    }
    out.push('\n');
    for suite in suites {
        out.push_str(&format!(
            "  <testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" errors=\"0\" skipped=\"0\" time=\"0\" timestamp=\"{}\">\n",
            escape(&suite.name),
            suite.tests,
            suite.failures,
            escape(&suite.timestamp)
        ));
        for case in &suite.cases {
            out.push_str(&format!(
                "    <testcase classname=\"{}\" name=\"{}\" time=\"{}\">",
                escape(&case.classname),
                escape(&case.name),
                go_g(case.time)
            ));
            if case.failed {
                out.push_str(
                    "\n      <failure message=\"Test failed\" type=\"failure\"></failure>\n    ",
                );
            }
            out.push_str("</testcase>\n");
        }
        out.push_str("  </testsuite>\n");
    }
    out.push_str("</testsuites>");
    out
}

/// The XML escaping Go's encoder applies to an attribute value.
pub(super) fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("&#34;"),
            '\'' => out.push_str("&#39;"),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\t' => out.push_str("&#x9;"),
            '\n' => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            other => out.push(other),
        }
    }
    out
}

/// Go's `strconv.FormatFloat(value, 'g', -1, 64)` spelling.
///
/// The shortest representation that round-trips, written in the decimal form
/// unless the exponent leaves the range the decimal form covers.
pub(super) fn go_g(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let scientific = format!("{value:e}");
    let Some((mantissa, exponent_text)) = scientific.split_once('e') else {
        return format!("{value}");
    };
    let Ok(exponent) = exponent_text.parse::<i32>() else {
        return format!("{value}");
    };
    if !(-4..6).contains(&exponent) {
        let sign = if exponent < 0 { '-' } else { '+' };
        let magnitude = exponent.abs();
        if magnitude < 10 {
            return format!("{mantissa}e{sign}0{magnitude}");
        }
        return format!("{mantissa}e{sign}{magnitude}");
    }
    format!("{value}")
}
