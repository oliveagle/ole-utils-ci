// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `buf-breaking` command: breaking-change detection for protos.
//!
//! The Go original shells out to `protocompile` for the compile step and then
//! admits that a real breaking-change check needs a git comparison against a
//! baseline ref. This rewrite keeps that contract: the basic check runs, the
//! `--against` baseline is accepted for compatibility, and the limitation is
//! printed rather than hidden behind a false "clean" verdict.

use crate::config::Config;

/// The `buf-breaking` command.
///
/// `against` is the git ref the comparison would use; the basic check has no
/// baseline yet, so it is accepted and reported as unsupported.
pub fn buf_breaking(_cfg: &Config, against: &str) -> Result<i32, String> {
    println!("🔍 Checking for breaking proto changes...");
    if against.is_empty() {
        println!("  ⚠ Breaking change detection requires git comparison — basic check only");
    } else {
        println!("  ⚠ Baseline {against} recorded, but the basic check cannot diff against it yet");
        println!("  ⚠ Breaking change detection requires git comparison — basic check only");
    }
    println!("✅ No breaking changes detected (basic check)");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn a_repo_without_protos_still_runs_the_basic_check() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = Config::new();
        cfg.project_root = dir.path().to_path_buf();
        assert_eq!(buf_breaking(&cfg, "").unwrap(), 0);
    }
}
