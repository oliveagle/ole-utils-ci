// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Validation of the `.devine-utils-ci.jsonc` file-length section.
//!
//! A project may tighten a budget. It may not raise one, switch a file to
//! another profile, or exclude a path from a hard budget.

use crate::config::limits;
use crate::quality_config::QualityConfig;

use super::profiles::default_profiles;

/// Rejects a config that tries to raise a budget or exclude a path.
pub fn validate_config(qcfg: &QualityConfig) -> Result<(), String> {
    reject_specific_ignore(qcfg, "file-length")?;
    let Some(fl) = &qcfg.file_length else {
        return Ok(());
    };
    validate_profiles(fl)?;
    validate_path_rules(fl)
}

/// Rejects a profile budget that raises a built-in ceiling.
fn validate_profiles(fl: &crate::quality_config::QualityFileLengthConfig) -> Result<(), String> {
    let defaults = default_profiles();
    for (name, limit) in &fl.profiles {
        let Some(base) = defaults.get(name.as_str()) else {
            return Err(format!("file-length: unknown profile {name:?}"));
        };
        if limit.warn > base.warn || limit.max > base.max {
            return Err(format!(
                "file-length: profile {name:?} can only tighten built-in warn={} max={}",
                base.warn, base.max
            ));
        }
    }
    Ok(())
}

/// Rejects a path rule that switches profile or raises the source ceiling.
fn validate_path_rules(fl: &crate::quality_config::QualityFileLengthConfig) -> Result<(), String> {
    for (pattern, rule) in &fl.paths {
        if !rule.mode.is_empty() || !rule.profile.is_empty() {
            return Err(format!(
                "file-length: path {pattern:?} cannot switch profile or exclude files"
            ));
        }
        if rule.warn > limits::HARD_SOURCE_MAX_LINES || rule.max > limits::HARD_SOURCE_MAX_LINES {
            return Err(format!(
                "file-length: path {pattern:?} can only tighten below the {}-line source/script ceiling",
                limits::HARD_SOURCE_MAX_LINES
            ));
        }
    }
    Ok(())
}

/// Rejects an ignore section for a check whose budget is a hard limit.
pub fn reject_specific_ignore(qcfg: &QualityConfig, check: &str) -> Result<(), String> {
    let section = match check {
        "file-length" => &qcfg.ignore.file_length,
        "complexity" => &qcfg.ignore.complexity,
        "package-size" => &qcfg.ignore.package_size,
        "artifact-size" => &qcfg.ignore.artifact_size,
        _ => return Ok(()),
    };
    if let Some(section) = section {
        if !section.paths.is_empty() {
            return Err(format!(
                "{check}: path ignores are not allowed for hard quality budgets; lower the budget instead"
            ));
        }
    }
    Ok(())
}
