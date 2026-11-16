// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The gates that turn sources into something else: a precompiled package and its documentation.
//!
//! Both commands take one input — a package directory for `precompile`, one path for `doc` — and
//! both write into a scratch tree, so a run leaves the project as it found it.

use std::path::Path;

use crate::commands::lang_tool;
use crate::commands::mojo::report::{absorb, emit_report, skip};
use crate::commands::mojo::scratch::{stem_of, Scratch};
use crate::commands::mojo::sources::Project;
use crate::commands::mojo::{program, Options, MOJO_HINT};
use crate::config::Config;
use crate::output::Report;

/// `mojo-package`: one precompile per package directory, into a scratch file.
///
/// `package` is the historical name and `precompile` the current one; 1.2 warns that the first is
/// deprecated. The gate asks for `precompile` and falls back to `package` for a toolchain older
/// than the rename, so it works on both without a version check of its own.
pub fn package(cfg: &Config, opts: &Options) -> Result<i32, String> {
    let project = Project::read(cfg.root());
    let packages = if opts.paths.is_empty() {
        project.packages.clone()
    } else {
        opts.paths.clone()
    };
    if packages.is_empty() {
        return skip(
            cfg,
            "mojo-package",
            &project,
            "no package directory (a directory holding `__init__.mojo`)",
        );
    }
    let scratch = Scratch::new("mojo-package")?;
    let mut report = Report::new("mojo-package");
    let mut packaged = 0;
    for dir in &packages {
        let name = stem_of(dir.trim_end_matches('/'));
        let output = scratch.path_of(&format!("{name}.mojoc"));
        let out = precompile(cfg, opts, dir, &output)?;
        if out.success {
            packaged += 1;
        } else {
            absorb(&mut report, &out.output, "mojo.package", dir);
        }
    }
    let summary = format!("{packaged} of {} package(s) precompiled", packages.len());
    Ok(emit_report(cfg, report, "mojo-package", &summary))
}

/// `mojo-doc`: one documentation build per input, because `doc` takes exactly one.
pub fn doc(cfg: &Config, opts: &Options) -> Result<i32, String> {
    let project = Project::read(cfg.root());
    let targets = if !opts.paths.is_empty() {
        opts.paths.clone()
    } else if !project.packages.is_empty() {
        // A package directory documents every module in it at once.
        project.packages.clone()
    } else {
        project.modules.clone()
    };
    if targets.is_empty() {
        return skip(cfg, "mojo-doc", &project, "no Mojo source to document");
    }
    let scratch = Scratch::new("mojo-doc")?;
    let mut report = Report::new("mojo-doc");
    let mut documented = 0;
    for target in &targets {
        let output = scratch.path_of(&format!("{}.json", stem_of(target.trim_end_matches('/'))));
        let args = vec![
            "doc".to_string(),
            target.clone(),
            "-o".to_string(),
            output.to_string_lossy().into_owned(),
        ];
        let out = lang_tool::run(cfg, &program(opts), &args, MOJO_HINT)?;
        if out.success {
            documented += 1;
        } else {
            absorb(&mut report, &out.output, "mojo.doc", target);
        }
    }
    let summary = format!("{documented} of {} input(s) documented", targets.len());
    Ok(emit_report(cfg, report, "mojo-doc", &summary))
}

/// Precompiles one package, tolerating the rename that split the toolchain in two.
fn precompile(
    cfg: &Config,
    opts: &Options,
    dir: &str,
    output: &Path,
) -> Result<crate::runner::CmdOutput, String> {
    let out = |verb: &str, suffix: &str| {
        let file = output.with_extension(suffix);
        let args = vec![
            verb.to_string(),
            dir.to_string(),
            "-o".to_string(),
            file.to_string_lossy().into_owned(),
        ];
        lang_tool::run(cfg, &program(opts), &args, MOJO_HINT)
    };
    let first = out("precompile", "mojoc")?;
    if first.success || !names_an_unknown_command(&first.output) {
        return Ok(first);
    }
    out("package", "mojopkg")
}

/// Whether a tool's complaint is "this build of me has no such subcommand".
fn names_an_unknown_command(text: &str) -> bool {
    let text = text.to_lowercase();
    text.contains("no such command")
        || text.contains("unknown command")
        || text.contains("unrecognized subcommand")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_subcommand_is_recognised() {
        assert!(names_an_unknown_command(
            "mojo: error: no such command 'test'"
        ));
        assert!(names_an_unknown_command(
            "/usr/bin/mojo: error: unrecognized subcommand"
        ));
        assert!(!names_an_unknown_command(
            "error: module does not contain a 'main' function"
        ));
    }
}
