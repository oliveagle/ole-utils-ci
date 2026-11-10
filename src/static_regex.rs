// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Compiling the regexes the gates match with.

use regex::Regex;

/// Compiles one of the gate's static patterns.
///
/// Every pattern is a literal the gate owns, so a compile error means the
/// gate itself is broken rather than the project under inspection. `expect`
/// turns that into one message at first use instead of an `Option` threaded
/// through every caller, and the `nolint` marks this as the one place the
/// gate accepts a panicking constructor.
pub fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static gate regex must compile") //nolint:rust/expect
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_a_static_pattern() {
        assert!(compile(r"^\d+$").is_match("42"));
        assert!(!compile(r"^\d+$").is_match("x"));
    }
}
