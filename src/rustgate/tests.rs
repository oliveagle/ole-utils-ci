// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the Rust gates.

use super::*;

#[test]
fn compares_versions() {
    assert!(version_gt("1.85.0", "1.84.9"));
    assert!(!version_gt("1.85.0", "1.85.0"));
    assert!(!version_gt("1.84.9", "1.85.0"));
}

#[test]
fn lists_every_rust_gate() {
    assert_eq!(GATES.len(), 10);
    assert!(GATES.contains(&"rust-clippy"));
}
