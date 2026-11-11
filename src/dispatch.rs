// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The command table: which subcommand runs which entry point, and which
//! commands get their private flags lifted out before the shared parser.

use devine_utils_ci::commands;
use devine_utils_ci::config::Config;
use devine_utils_ci::version;

use crate::{print_usage, GATE_COMMANDS};

/// Runs the requested subcommand.
pub fn dispatch(command: &str, args: &[String]) -> Result<i32, String> {
    if matches!(command, "help" | "-h" | "--help") {
        print_usage();
        return Ok(0);
    }
    if matches!(command, "version" | "--version") {
        println!("{}", devine_utils_ci::version_banner());
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
        "repo-status" => with_config(commands::repo::repo_status, &mut cfg, args),
        "cleanup" => with_config(commands::repo::cleanup, &mut cfg, args),
        "ci-lint" | "glab-ci-lint" => with_config(commands::ci::ci_lint, &mut cfg, args),
        "arch" => with_config(commands::arch::arch, &mut cfg, args),
        "buf-lint" => with_config(commands::proto::buf_lint, &mut cfg, args),
        "web-lint" => with_config(commands::web::web_lint, &mut cfg, args),
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
        // Go hands `logs` its raw args: the command owns a private flag set
        // the shared parser must not see.
        "logs" => commands::logs::logs(args),
        // Go hoists `captain-ci-test`'s own flags before handing the rest to
        // the shared parser, so one line can mix both sets.
        "captain-ci-test" => captain_ci_command(&mut cfg, args),
        "report-bundle" => with_config(commands::reports::report_bundle, &mut cfg, args),
        "verify-dist" => with_config(commands::dist::verify_dist, &mut cfg, args),
        "bump-version" => bump_command(&mut cfg, args, false),
        "release-start" => bump_command(&mut cfg, args, true),
        other => Err(format!("unknown command: {other}")),
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

/// Parses the bump type for `bump-version` and `release-start`.
pub fn bump_command(cfg: &mut Config, args: &[String], release: bool) -> Result<i32, String> {
    let rest = commands::gates::parse(cfg, args)?;
    let raw = rest.first().map(String::as_str).unwrap_or("");
    let bump = version::parse_bump(raw)?;
    if release {
        commands::version::release::release_start(cfg, bump)
    } else {
        commands::version::bump::bump_version(cfg, bump)
    }
}
