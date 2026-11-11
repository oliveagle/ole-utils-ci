// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The one human-readable rendering `doctor` has.
//!
//! The Go original never routes this command through `output.Report`: the
//! report is read by a person, so there is no structured mode to render and
//! `--output` changes nothing. The whole report is built as one string
//! first, which keeps the tests free of a stdout capture.

use super::checks::Check;

/// The width of the two rules that frame the report.
const RULE: usize = 60;

/// Prints the report.
pub(super) fn report(results: &[Check]) {
    print!("{}", text(results));
}

/// The whole report, ready to print.
pub(super) fn text(results: &[Check]) -> String {
    let mut out = String::new();
    out.push_str("🔍 devine-utils-ci doctor\n");
    out.push_str(&rule());
    let (mut pass, mut warn, mut fail) = (0, 0, 0);
    for result in results {
        out.push_str(&format!("\n{}  {}\n", icon(result), result.name));
        if !result.detail.is_empty() {
            out.push_str(&format!("   {}\n", result.detail));
        }
        if !result.hint.is_empty() {
            out.push_str(&format!("   💡 {}\n", result.hint));
        }
        match result.status {
            "pass" => pass += 1,
            "warn" => warn += 1,
            _ => fail += 1,
        }
    }
    out.push_str(&rule());
    out.push_str(&format!(
        "📊 {pass} passed, {warn} warnings, {fail} failures\n"
    ));
    out.push_str(&verdict(fail, warn));
    out
}

/// The one emoji a status maps to, the way Go's table does.
fn icon(result: &Check) -> &'static str {
    match result.status {
        "pass" => "✅",
        "warn" => "⚠️",
        _ => "❌",
    }
}

/// The rule with its newline.
fn rule() -> String {
    format!("{}\n", "─".repeat(RULE))
}

/// The last line: the verdict, worst status first.
fn verdict(fail: usize, warn: usize) -> String {
    if fail > 0 {
        "❌ Some checks failed — review the hints above.\n".to_string()
    } else if warn > 0 {
        "⚠️ All critical checks passed, but review warnings.\n".to_string()
    } else {
        "✅ Environment looks healthy.\n".to_string()
    }
}
