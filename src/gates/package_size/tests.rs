// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the package-size gate.

use super::*;

#[test]
fn counts_go_exported_symbols() {
    let src = "func Exported() {}\nfunc unexported() {}\ntype Public struct{}\n";
    assert_eq!(count_exported_symbols(src, ".go"), 2);
}

#[test]
fn counts_rust_exported_symbols() {
    let src = "pub fn a() {}\npub(crate) fn b() {}\nfn c() {}\npub struct D;\n";
    assert_eq!(count_exported_symbols(src, ".rs"), 2);
}

#[test]
fn collects_imports() {
    let go = "import (\n\t\"fmt\"\n\t\"os\"\n)\n";
    assert_eq!(collect_imports(go, ".go").len(), 2);
    let rs = "use std::fs;\nuse crate::x;\n";
    assert_eq!(collect_imports(rs, ".rs").len(), 2);
}
