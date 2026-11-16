// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The flag validation of `rhai-check`.

use super::flags::RhaiFlags;

/// Rejects budgets and thresholds that cannot describe a gate.
pub(super) fn validate(flags: &RhaiFlags) -> Result<(), String> {
    if flags.max_files < 1 {
        return Err("--max-files must be at least 1".to_string());
    }
    if flags.max_file_size < 1 {
        return Err("--max-file-size must be at least 1".to_string());
    }
    if !(0.0..=100.0).contains(&flags.line_threshold)
        || !(0.0..=100.0).contains(&flags.branch_threshold)
    {
        return Err("coverage thresholds must be between 0 and 100".to_string());
    }
    if flags.complexity_warn == 0
        || flags.max_complexity == 0
        || flags.complexity_warn > flags.max_complexity
    {
        return Err("complexity budgets must satisfy 0 < warn <= max".to_string());
    }
    Ok(())
}
