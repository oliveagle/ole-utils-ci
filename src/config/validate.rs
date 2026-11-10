// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The "thresholds may only be tightened" rule.
//!
//! Every budget the tool enforces has a hard ceiling. A CLI flag or a
//! project configuration may lower a budget; asking for a larger one is a
//! configuration error, not a warning.

use crate::config::{limits, output, Config};

impl Config {
    /// Enforces the "thresholds may only be tightened" rule.
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.validate_coverage_thresholds()?;
        self.validate_file_lines()?;
        self.validate_complexity()?;
        output::validate_mode(&self.output)?;
        Ok(())
    }

    /// The coverage floors cannot be lowered.
    fn validate_coverage_thresholds(&self) -> Result<(), String> {
        if self.coverage_threshold < limits::COVERAGE_FLOOR {
            return Err(format!(
                "--coverage-threshold={} is below the floor of {}% — threshold can only be raised, never lowered",
                self.coverage_threshold,
                limits::COVERAGE_FLOOR
            ));
        }
        if self.branch_coverage_threshold < limits::COVERAGE_FLOOR {
            return Err(format!(
                "--branch-coverage-threshold={} is below the floor of {}% — threshold can only be raised, never lowered",
                self.branch_coverage_threshold,
                limits::COVERAGE_FLOOR
            ));
        }
        Ok(())
    }

    /// The file-line ceiling cannot be raised.
    fn validate_file_lines(&self) -> Result<(), String> {
        if self.max_file_lines > limits::HARD_SOURCE_MAX_LINES {
            return Err(format!(
                "--max-file-lines={} exceeds the cap of {} — file line limit can only be lowered, never raised",
                self.max_file_lines,
                limits::HARD_SOURCE_MAX_LINES
            ));
        }
        if self.max_file_lines == 0 {
            return Err(format!(
                "--max-file-lines={} must be positive",
                self.max_file_lines
            ));
        }
        Ok(())
    }

    /// The complexity ceilings cannot be raised.
    fn validate_complexity(&self) -> Result<(), String> {
        if self.max_complexity > limits::HARD_COMPLEXITY_MAX {
            return Err(format!(
                "--max-complexity={} exceeds the cap of {} — complexity threshold can only be lowered, never raised",
                self.max_complexity,
                limits::HARD_COMPLEXITY_MAX
            ));
        }
        if self.complexity_warn == 0 || self.complexity_warn > self.max_complexity {
            return Err(format!(
                "--complexity-warn={} must be between 1 and --max-complexity={}",
                self.complexity_warn, self.max_complexity
            ));
        }
        if self.complexity_warn > limits::HARD_COMPLEXITY_WARN {
            return Err(format!(
                "--complexity-warn={} exceeds the built-in warning ceiling of {} — thresholds can only be tightened",
                self.complexity_warn,
                limits::HARD_COMPLEXITY_WARN
            ));
        }
        Ok(())
    }
}
