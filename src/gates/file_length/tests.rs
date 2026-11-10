// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the file-length gate.

use super::*;

#[test]
fn maps_extensions_to_profiles() {
    assert_eq!(profile_for_extension(".rs"), Some("source"));
    assert_eq!(profile_for_extension(".sh"), Some("script"));
    assert_eq!(profile_for_extension(".html"), Some("html"));
    assert_eq!(profile_for_extension(".yaml"), Some("declarative"));
    assert_eq!(profile_for_extension(".bin"), None);
}

#[test]
fn built_in_budgets_are_the_documented_ones() {
    let p = default_profiles();
    assert_eq!(p["source"].max, 500);
    assert_eq!(p["source"].warn, 400);
    assert_eq!(p["html"].max, 1200);
    assert_eq!(p["declarative"].max, 1000);
}
