// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The gates that drive the `mojo` binary — written against the CLI it actually offers.
//!
//! The first version of this module assumed a CLI that does not exist in Mojo 1.2, and every
//! assumption was a gate that could not pass:
//!
//! * `mojo format --check` is an unrecognised argument — the formatter has no check mode and
//!   rewrites in place, so a *check* has to run it on a copy and compare;
//! * `mojo test` is not a command (the CLI has run/build/repl/debug/precompile/format/doc);
//! * `build` and `doc` take exactly one input — "too many input files, cannot process both"
//!   — and `precompile` (the command `package` was renamed to) takes one package directory.
//!
//! So each gate computes the one shape its tool wants: a scratch copy for the formatter, one
//! invocation per entry point for the compiler, one per test file for the runner, one per package
//! for the precompiler. Nothing the gates run writes into the project.

use crate::commands::lang_tool;
use crate::commands::mojo::report::{absorb, emit_report, skip};
use crate::commands::mojo::scratch::{first_difference, stem_of, Scratch};
use crate::commands::mojo::sources::{self, Project};
use crate::commands::mojo::{program, Options, MOJO_HINT};
use crate::config::Config;
use crate::output::{Diagnostic, Report};

/// The modules a gate acts on: what the caller named, else every `.mojo` file the project has.
fn modules(cfg: &Config, opts: &Options) -> Vec<String> {
    if !opts.paths.is_empty() {
        return opts.paths.clone();
    }
    sources::Project::read(cfg.root()).modules
}

/// `mojo-format`: the layout, checked without letting the formatter touch the sources.
///
/// `mojo format` has no check mode, so the gate formats a scratch copy of every module and
/// compares. Same answer a `--check` would give, and the project is never written to — which is
/// the whole point of running a check on someone else's tree in CI.
pub fn format(cfg: &Config, opts: &Options) -> Result<i32, String> {
    let files = modules(cfg, opts);
    if files.is_empty() {
        let project = Project::read(cfg.root());
        return skip(cfg, "mojo-format", &project, "no Mojo source to format");
    }
    if cfg.dry_run {
        // No copy exists yet, so the plan names the sources and the summary says what happens to
        // them: the formatter will be pointed at a scratch tree, not at this project.
        let mut plan = vec!["format".to_string()];
        plan.extend(files.iter().cloned());
        let out = lang_tool::run(cfg, &program(opts), &plan, MOJO_HINT)?;
        let summary = format!("{} source(s) formatted on a copy", files.len());
        return Ok(crate::gates::emit(
            cfg,
            &lang_tool::verdict(cfg, "mojo-format", summary, &out),
        ));
    }
    let scratch = Scratch::new("mojo-format")?;
    let mut copies = Vec::with_capacity(files.len());
    for rel in &files {
        copies.push(scratch.copy_in(cfg.root(), rel)?);
    }
    let mut args = vec!["format".to_string()];
    args.extend(copies);
    let out = lang_tool::run(cfg, &program(opts), &args, MOJO_HINT)?;
    if !out.success {
        // The formatter refused a file: its own findings are the answer, one per file it named.
        return Ok(crate::gates::emit(
            cfg,
            &lang_tool::verdict(cfg, "mojo-format", "already formatted", &out),
        ));
    }
    let mut report = Report::new("mojo-format");
    for rel in &files {
        let original = cfg.root().join(rel.as_str());
        let copy = scratch.path_of(rel);
        if crate::commands::mojo::scratch::changed(&original, &copy) {
            let line = first_difference(
                &std::fs::read(&original).unwrap_or_default(),
                &std::fs::read(&copy).unwrap_or_default(),
            );
            report.add(
                Diagnostic::error(
                    rel.clone(),
                    "mojo.unformatted",
                    "`mojo format` would rewrite this file",
                )
                .at_line(line),
            );
        }
    }
    Ok(emit_report(cfg, report, "mojo-format", "already formatted"))
}

/// `mojo-build`: one invocation per entry point, one scratch output each.
pub fn build(cfg: &Config, opts: &Options) -> Result<i32, String> {
    let project = Project::read(cfg.root());
    let entries = if opts.paths.is_empty() {
        project.entry_points(cfg.root())
    } else {
        opts.paths.clone()
    };
    if entries.is_empty() {
        return skip(
            cfg,
            "mojo-build",
            &project,
            "no `def main` entry point to build",
        );
    }
    let scratch = Scratch::new("mojo-build")?;
    let mut report = Report::new("mojo-build");
    let mut built = 0;
    for rel in &entries {
        let output = scratch.path_of(&format!("{}.bin", stem_of(rel)));
        let args = vec![
            "build".to_string(),
            rel.clone(),
            "-o".to_string(),
            output.to_string_lossy().into_owned(),
        ];
        let out = lang_tool::run(cfg, &program(opts), &args, MOJO_HINT)?;
        if out.success {
            built += 1;
        } else {
            absorb(&mut report, &out.output, "mojo.build", rel);
        }
    }
    let summary = format!("{built} of {} entry point(s) built", entries.len());
    Ok(emit_report(cfg, report, "mojo-build", &summary))
}

/// `mojo-test`: every test file run, because the toolchain has no test command.
///
/// A Mojo test is a program that asserts and exits: `mojo run <file>` is the whole runner, and
/// the exit status is the verdict.
pub fn test(cfg: &Config, opts: &Options) -> Result<i32, String> {
    let project = Project::read(cfg.root());
    let tests = if opts.paths.is_empty() {
        project.tests()
    } else {
        opts.paths.clone()
    };
    if tests.is_empty() {
        return skip(cfg, "mojo-test", &project, "no `test_*.mojo` file to run");
    }
    let mut report = Report::new("mojo-test");
    let mut passed = 0;
    for rel in &tests {
        let args = vec!["run".to_string(), rel.clone()];
        let out = lang_tool::run(cfg, &program(opts), &args, MOJO_HINT)?;
        if out.success {
            passed += 1;
        } else {
            absorb(&mut report, &out.output, "mojo.test", rel);
        }
    }
    let summary = format!("{passed} of {} test file(s) passed", tests.len());
    Ok(emit_report(cfg, report, "mojo-test", &summary))
}
