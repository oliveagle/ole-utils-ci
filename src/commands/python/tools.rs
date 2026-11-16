// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The Python toolchain gates: the ones that drive the project's own tools.
//!
//! Each is looked up on `PATH`, may be overridden with `--tool` (or, for the
//! interpreter, `--interpreter`), and reports by name when it is missing. A
//! gate whose tool never ran is an error, never a pass — an unrun check proves
//! nothing, and a project that forgot to install `ruff` must not read as a
//! project `ruff` passed.

use crate::commands::lang_tool;
use crate::config::Config;

use super::Options;

/// The install line the "tool not found" complaint carries.
const RUFF_HINT: &str = "install it (`pip install ruff`) or pass --tool";
const MYPY_HINT: &str = "install it (`pip install mypy`) or pass --tool";
const PYTEST_HINT: &str = "install pytest (`pip install pytest`) or pass --interpreter";
const COV_HINT: &str = "install pytest-cov (`pip install pytest-cov`) or pass --interpreter";
const BUILD_HINT: &str = "install the build frontend (`pip install build`) or pass --interpreter";

/// Ruff's own line-length opinions, as a check.
pub(super) fn ruff_check(cfg: &Config, opts: &Options) -> Result<i32, String> {
    let program = opts.ruff_program();
    let mut args = vec!["check".to_string(), "--output-format=concise".to_string()];
    push_config(&mut args, opts);
    args.extend(opts.paths());
    args.extend(opts.tool_args.iter().cloned());
    let out = lang_tool::run(cfg, &program, &args, RUFF_HINT)?;
    let result = lang_tool::verdict(cfg, "python-lint", "ruff found nothing", &out);
    Ok(emit(cfg, &result))
}

/// The formatter's own idea of the canonical layout, as a check.
pub(super) fn ruff_format(cfg: &Config, opts: &Options) -> Result<i32, String> {
    let program = opts.ruff_program();
    let mut args = vec![
        "format".to_string(),
        "--check".to_string(),
        "--diff".to_string(),
    ];
    push_config(&mut args, opts);
    args.extend(opts.paths());
    args.extend(opts.tool_args.iter().cloned());
    let out = lang_tool::run(cfg, &program, &args, RUFF_HINT)?;
    let result = lang_tool::verdict(cfg, "python-format", "already formatted", &out);
    Ok(emit(cfg, &result))
}

/// The type annotations the checker can prove, as a check.
pub(super) fn mypy(cfg: &Config, opts: &Options) -> Result<i32, String> {
    let program = opts.mypy_program();
    let mut args = vec![
        "--no-error-summary".to_string(),
        "--hide-error-context".to_string(),
    ];
    if !opts.config.is_empty() {
        args.push("--config-file".to_string());
        args.push(opts.config.clone());
    }
    args.extend(opts.paths());
    args.extend(opts.tool_args.iter().cloned());
    let out = lang_tool::run(cfg, &program, &args, MYPY_HINT)?;
    let result = lang_tool::verdict(cfg, "python-typecheck", "mypy is clean", &out);
    Ok(emit(cfg, &result))
}

/// The coverage gate needs pytest-cov, and pytest does not say so.
///
/// Without the plugin, pytest answers a `--cov` line with "unrecognized
/// arguments: --cov", under its own usage banner — a wall of text that names
/// neither the plugin nor the fix. Asking the interpreter for the module first
/// turns that into one sentence.
fn require_pytest_cov(cfg: &Config, opts: &Options) -> Result<(), String> {
    // A dry run gates nothing, so it must not need the plugin either.
    if cfg.dry_run {
        return Ok(());
    }
    let args = vec!["-c".to_string(), "import pytest_cov".to_string()];
    let out = lang_tool::run(cfg, &opts.interpreter, &args, COV_HINT)?;
    if out.success {
        return Ok(());
    }
    Err(format!(
        "{} cannot import pytest_cov — {COV_HINT}",
        opts.interpreter
    ))
}

/// The test suite, with or without the coverage floor.
pub(super) fn pytest(cfg: &Config, opts: &Options, coverage: bool) -> Result<i32, String> {
    let gate = if coverage {
        "python-coverage"
    } else {
        "python-test"
    };
    if coverage {
        require_pytest_cov(cfg, opts)?;
    }
    let mut args = vec!["-m".to_string(), "pytest".to_string(), "-q".to_string()];
    if !cfg.junit_path.is_empty() {
        args.push(format!("--junit-xml={}", cfg.junit_path));
    }
    if coverage {
        args.push("--cov".to_string());
        args.push("--cov-report=term-missing".to_string());
        args.push(format!("--cov-fail-under={}", cfg.coverage_threshold));
    }
    args.extend(opts.paths());
    args.extend(opts.tool_args.iter().cloned());
    let hint = if coverage { COV_HINT } else { PYTEST_HINT };
    let out = lang_tool::run(cfg, &opts.interpreter, &args, hint)?;
    let ok = if coverage {
        format!(
            "every test passed, coverage at or above {}%",
            cfg.coverage_threshold
        )
    } else {
        "every test passed".to_string()
    };
    let result = lang_tool::verdict(cfg, gate, ok, &out);
    Ok(emit(cfg, &result))
}

/// The distribution the project would publish.
pub(super) fn build(cfg: &Config, opts: &Options) -> Result<i32, String> {
    let mut args = vec!["-m".to_string(), "build".to_string()];
    args.extend(opts.tool_args.iter().cloned());
    let out = lang_tool::run(cfg, &opts.interpreter, &args, BUILD_HINT)?;
    let result = lang_tool::verdict(cfg, "python-build", "sdist and wheel built", &out);
    Ok(emit(cfg, &result))
}

/// Appends `--config <path>` when the caller named one.
fn push_config(args: &mut Vec<String>, opts: &Options) {
    if !opts.config.is_empty() {
        args.push("--config".to_string());
        args.push(opts.config.clone());
    }
}

/// Prints a gate result and returns its exit code.
fn emit(cfg: &Config, result: &crate::gates::GateResult) -> i32 {
    crate::gates::emit(cfg, result)
}
