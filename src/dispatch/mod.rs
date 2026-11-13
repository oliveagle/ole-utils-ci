// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The command table: which subcommand runs which entry point, and which
//! commands get their private flags lifted out before the shared parser.
//!
//! The table has two layers. The flat layer is the command surface the Go
//! original had. The language layer sits in front of it: `rust`, `golang`,
//! `tengo`, `proto`, `web` and `common` group the flat commands by the
//! language they gate and forward to them (see [`crate::languages`]).

mod flat;

use crate::commands;
use crate::config::Config;
use crate::gates;
use crate::languages;
use crate::usage;

pub use flat::{has_flat_command, FLAT_COMMANDS};

/// The commands that need nothing but the global flags.
pub const GATE_COMMANDS: [&str; 9] = [
    "bugs",
    "sanity",
    "license",
    "structure",
    "file-length",
    "complexity",
    "package-size",
    "artifact-size",
    "goversion",
];

/// Runs the requested subcommand.///
/// A language group is resolved first: it forwards to the flat command that
/// does the work, so `rust clippy` and `rust-clippy` run the same gate.
pub fn dispatch(command: &str, args: &[String]) -> Result<i32, String> {
    if let Some(lang) = languages::find(command) {
        return languages::run(lang, args);
    }
    dispatch_flat(command, args)
}

/// Runs a flat subcommand, without the language-group indirection.
pub(crate) fn dispatch_flat(command: &str, args: &[String]) -> Result<i32, String> {
    if matches!(command, "help" | "-h" | "--help") {
        usage::print();
        return Ok(0);
    }
    if matches!(command, "version" | "--version") {
        println!("{}", crate::version_banner());
        return Ok(0);
    }
    let mut cfg = Config::new();
    if GATE_COMMANDS.contains(&command) {
        commands::gates::parse(&mut cfg, args)?;
        return commands::gates::run_gate(command, &cfg);
    }
    if commands::rust::COMMANDS.contains(&command) {
        commands::rust::parse(&mut cfg, args)?;
        return commands::rust::run(command, &cfg);
    }
    match command {
        "install" => with_config(commands::toolchain::install, &mut cfg, args),
        "lint" => with_config(commands::toolchain::lint, &mut cfg, args),
        "test" => with_config(commands::toolchain::test, &mut cfg, args),
        "build" => with_config(commands::toolchain::build, &mut cfg, args),
        "coverage" => with_config(commands::toolchain::coverage, &mut cfg, args),
        "test-report" => with_config(commands::toolchain::test_report, &mut cfg, args),
        "branch-coverage" => {
            with_config(commands::branch_coverage::branch_coverage, &mut cfg, args)
        }
        // Go hands `gomut` and the `tengo-*` commands their raw args: each
        // owns a private flag set the shared parser must not see.
        "gomut" => commands::gomut::gomut(args),
        "tengo-test" => commands::tengo::tengo_test(args),
        "tengo-coverage" => commands::tengo::tengo_coverage(args),
        "tengo-mutate" => commands::tengo::tengo_mutate(args),
        "all" => with_config(commands::all::run, &mut cfg, args),
        "version-check" => with_config(commands::version::version_check, &mut cfg, args),
        "release-check" => with_config(commands::version::release_check, &mut cfg, args),
        "release-finish" => with_config(commands::version::release::release_finish, &mut cfg, args),
        "doctor" => with_config(commands::doctor::doctor, &mut cfg, args),
        "repo-status" => commands::repo_status::repo_status(args),
        "cleanup" => commands::cleanup::cleanup(args),
        "ci-lint" | "glab-ci-lint" => with_config(commands::ci::ci_lint, &mut cfg, args),
        "arch" => with_config(commands::arch::arch, &mut cfg, args),
        "buf-lint" => with_config(commands::proto::buf_lint, &mut cfg, args),
        // Go hands `web-lint` its raw args: the command owns a private flag
        // set the shared parser must not see.
        "web-lint" => commands::web::web_lint(args),
        // Go runs the focused lint gates with their own private flag sets,
        // so their flags never reach the shared parser.
        "go-pin" => gates::gopin::focused_command(args),
        "no-shell-exec" => gates::noshell::focused_command(args),
        "plugin-freshness" => gates::plugin_freshness::focused_command(args),
        "release" => with_config(commands::release_cmd::release, &mut cfg, args),
        "release-model" => with_config(commands::release_model::release_model, &mut cfg, args),
        "buf-generate" => with_config(commands::proto::buf_generate, &mut cfg, args),
        "buf-breaking" => buf_breaking_command(&mut cfg, args),
        "ci-go-env" | "ci_go_env" => go_env_command(&mut cfg, args),
        "git-author" => git_author_command(&mut cfg, args),
        // Go hands `ci-recipe` its raw args: each subcommand owns a private
        // flag set the shared parser must not see.
        "ci-recipe" => commands::ci_recipe_cmd::ci_recipe(args),
        // Go hands `protect-set` its raw args: the command owns a flat flag
        // list the shared parser must not see.
        "protect-set" => commands::protect_set::protect_set(args),
        // Go hands `upgrade` its raw args: the command owns a private flag
        // set the shared parser must not see.
        "upgrade" => commands::upgrade::upgrade(args),
        // Go hands `kms-stub` its raw args: the command owns a private flag
        // set and the global flags must not reach it.
        "kms-stub" => commands::kms_stub_cmd::kms_stub(args),
        // Not `with_config`: Go `runFakeMetadata` ignores the global flags
        // and owns its own `--run` / `-f` switch.
        "fake-metadata-server" => commands::fake_metadata::fake_metadata(args),
        // Go hands `pipeline-run` its raw args: the command owns a private
        // flag set the shared parser must not see.
        "pipeline-run" => commands::pipeline_run::pipeline_run(args),
        // Go hands `pipeline-watch` its raw args: the command owns a private
        // flag set the shared parser must not see.
        "pipeline-watch" => commands::pipeline_watch::pipeline_watch(args),
        // Go hands `logs` its raw args: the command owns a private flag set
        // the shared parser must not see.
        "logs" => commands::logs::logs(args),
        // Go hands `workflow` its raw args: every subcommand owns a private
        // flag list the shared parser must not see.
        "workflow" => commands::workflow::workflow(args),
        // Go hoists `captain-ci-test`'s own flags before handing the rest to
        // the shared parser, so one line can mix both sets.
        "captain-ci-test" => captain_ci_command(&mut cfg, args),
        // Go hands `report-bundle` its raw args: the command owns a private
        // flag set the shared parser must not see.
        "report-bundle" => commands::reports::report_bundle(args),
        // Go hands `verify-dist` its raw args: there is no flag set at all,
        // so the first argument is the directory to check.
        "verify-dist" => commands::dist::verify_dist(args),
        "bump-version" => bump_command(&mut cfg, args, false),
        "release-start" => bump_command(&mut cfg, args, true),
        // Go prints the complaint and the usage block on stdout and exits 1,
        // rather than routing it through the `Error:` prefix on stderr.
        other => {
            println!("Unknown command: {other}\n");
            usage::print();
            Ok(1)
        }
    }
}

/// Lifts `--name [value]` (and `--name=value`) out of the argument list.
///
/// Commands with flags the shared parser does not know about hoist them
/// first, the way the Go original hoists `--file` and `--config`, so a later
/// `--output` is never silently dropped.
pub fn hoist_flag(args: &[String], name: &str, default: &str) -> (String, Vec<String>) {
    let long = format!("--{name}");
    let mut value = default.to_string();
    let mut rest = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == &long && index + 1 < args.len() {
            value = args[index + 1].clone();
            index += 2;
            continue;
        }
        if let Some(inline) = arg.strip_prefix(&format!("{long}=")) {
            value = inline.to_string();
            index += 1;
            continue;
        }
        rest.push(arg.clone());
        index += 1;
    }
    (value, rest)
}

/// Runs `ci-go-env` with its `--file` flag lifted out first.
pub fn go_env_command(cfg: &mut Config, args: &[String]) -> Result<i32, String> {
    let (env_file, rest) = hoist_flag(args, "file", commands::goenv::DEFAULT_ENV_FILE);
    commands::gates::parse(cfg, &rest)?;
    commands::goenv::go_env(cfg, &env_file)
}

/// Runs `buf-breaking` with its `--against` baseline lifted out first.
pub fn buf_breaking_command(cfg: &mut Config, args: &[String]) -> Result<i32, String> {
    let (against, rest) = hoist_flag(args, "against", "");
    commands::gates::parse(cfg, &rest)?;
    commands::proto::buf_breaking(cfg, &against)
}

/// Runs `captain-ci-test`: its own flags come out before the shared parser.
pub fn captain_ci_command(cfg: &mut Config, args: &[String]) -> Result<i32, String> {
    let (options, rest) = commands::captain_ci::split_args(args);
    commands::gates::parse(cfg, &rest)?;
    commands::captain_ci::captain_ci_test(cfg, options)
}

/// Runs `git-author`: the sub-command and `--config` come out before the
/// shared parser sees the rest.
pub fn git_author_command(cfg: &mut Config, args: &[String]) -> Result<i32, String> {
    let (sub, config_path, rest) = commands::git_author_cmd::split_args(args);
    commands::gates::parse(cfg, &rest)?;
    commands::git_author_cmd::git_author(cfg, &sub, &config_path)
}

/// Parses the global flags for a command, then runs it.
pub fn with_config(
    run: fn(&Config) -> Result<i32, String>,
    cfg: &mut Config,
    args: &[String],
) -> Result<i32, String> {
    commands::gates::parse(cfg, args)?;
    run(cfg)
}

/// Resolves the bump type for `bump-version` and `release-start`.
///
/// `bump-version` hoists `--bump <action>` out before the shared parser
/// sees it, because Go's `runBumpVersion` does the same and lets a wrapper
/// pass the named flag instead of a positional. `release-start` has no such
/// flag, so its args go over untouched and the flag is rejected the way Go
/// rejects it. The text is parsed by the command itself: Go only validates
/// it after its banner, its project-config load and its `VERSION` read.
pub fn bump_command(cfg: &mut Config, args: &[String], release: bool) -> Result<i32, String> {
    let mut explicit = None;
    let mut rest: Vec<String> = Vec::with_capacity(args.len());
    if release {
        rest.extend_from_slice(args);
    } else {
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if arg == "--bump" && index + 1 < args.len() {
                explicit = Some(args[index + 1].clone());
                index += 2;
                continue;
            }
            if let Some(value) = arg.strip_prefix("--bump=") {
                explicit = Some(value.to_string());
            } else {
                rest.push(arg.clone());
            }
            index += 1;
        }
    }
    let positional = commands::gates::parse(cfg, &rest)?;
    let raw = match explicit {
        Some(value) => value,
        None if release => positional
            .first()
            .cloned()
            .unwrap_or_else(|| "patch".to_string()),
        None => {
            let first = positional.first().map(String::as_str).unwrap_or("");
            if first.is_empty() || first.starts_with('-') {
                "patch".to_string()
            } else {
                first.to_string()
            }
        }
    };
    if release {
        commands::version::release::release_start(cfg, &raw)
    } else {
        commands::version::bump::bump_version(cfg, &raw)
    }
}
