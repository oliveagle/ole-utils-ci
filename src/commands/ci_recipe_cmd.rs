// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `ci-recipe` command line: subcommands, private flags and usage.
//!
//! Go hands this command its raw arguments: `ci-recipe` owns a private
//! `flag.FlagSet` per subcommand (and none at all for `list`), so the
//! shared global parser must never see them.

use super::ci_recipe::{
    CIRecipe, DEFAULT_ARTIFACT, DEFAULT_BASE_URL, DEFAULT_PROJECT_ID, DEFAULT_RELEASE_JOB,
    SUPPORTED_ARTIFACTS,
};
use super::ci_recipe_util::download_ci_recipe_binary;
use crate::cli::{report_parse_error, FlagSet};

/// The usage block `ci-recipe` prints for no arguments, for `help` and
/// for `-h` as the subcommand — byte-identical to the Go original.
const USAGE: &str = r#"ci-recipe — render / download the artifact recipe downstream repos use

Usage:
  devine-utils-ci ci-recipe url      --version vX.Y.Z [--artifact <name>]
  devine-utils-ci ci-recipe script   --version vX.Y.Z [--artifact <name>] [--install-dir <dir>]
  devine-utils-ci ci-recipe download --version vX.Y.Z [--artifact <name>] [--install-dir <dir>]
                                     [--job-token <token> | $CI_JOB_TOKEN]
  devine-utils-ci ci-recipe list

Flags (all subcommands):
  --version    <vX.Y.Z>          Upstream release tag (mandatory)
  --artifact   <filename>        One of ci-recipe list (default: linux-amd64)
  --install-dir <dir>            Where to put the binary (default: bin)
  --project-id <id>              Override default upstream project id
  --base-url   <url>             Override default GitLab base URL
  --job-name   <name>            Override the upstream release job name
  --no-verify                    Skip the post-install version check

Examples:
  # What URL does downstream CI hit?
  devine-utils-ci ci-recipe url --version v0.2.6

  # What script should the setup: job run?
  devine-utils-ci ci-recipe script --version v0.2.6

  # Verify the recipe works end-to-end from your laptop before opening an MR:
  CI_JOB_TOKEN=$(dotvault get DEVINE_GIT_GROUP_ACCESS_KEY) \
    devine-utils-ci ci-recipe download --version v0.2.6 --install-dir /tmp
"#;

/// The `devine-utils-ci ci-recipe <subcommand>` entry point.
///
/// Returns the process exit code; every refusal travels up as an error
/// so `main` can print its own `Error:` line, exactly like Go's
/// `runCIRecipe` calling `fail`.
pub fn ci_recipe(args: &[String]) -> Result<i32, String> {
    let Some((sub, rest)) = args.split_first() else {
        print!("{USAGE}");
        return Ok(0);
    };
    match sub.as_str() {
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            Ok(0)
        }
        "url" => url_command(rest),
        "script" => script_command(rest),
        "download" => download_command(rest),
        "list" => {
            for artifact in SUPPORTED_ARTIFACTS {
                println!("{artifact}");
            }
            Ok(0)
        }
        other => Err(format!(
            "ci-recipe: unknown subcommand {other:?} (see `ci-recipe help`)"
        )),
    }
}

/// `ci-recipe url`: print the artifact download URL.
fn url_command(args: &[String]) -> Result<i32, String> {
    let recipe = parse_flags("url", args)?;
    println!("{}", recipe.artifact_url()?);
    Ok(0)
}

/// `ci-recipe script`: print the downstream `setup:` job's script.
fn script_command(args: &[String]) -> Result<i32, String> {
    let recipe = parse_flags("script", args)?;
    print!("{}", recipe.setup_script()?);
    Ok(0)
}

/// `ci-recipe download`: fetch, gunzip, chmod and verify locally.
fn download_command(args: &[String]) -> Result<i32, String> {
    let recipe = parse_flags("download", args)?;
    if recipe.job_token.is_empty() {
        return Err("--job-token missing and $CI_JOB_TOKEN unset".to_string());
    }
    let installed = download_ci_recipe_binary(&recipe)?;
    println!("{}", installed.trim());
    Ok(0)
}

/// The flag set every subcommand but `list` shares, registered the way
/// Go's `parseCIRecipeFlags` does — including the `CI_JOB_TOKEN` default
/// that `PrintDefaults` quotes back at the user.
fn build_flags() -> FlagSet {
    let mut flags = FlagSet::new();
    flags.str_flag("version", "");
    flags.str_flag("artifact", DEFAULT_ARTIFACT);
    flags.str_flag("install-dir", "bin");
    flags.str_flag("project-id", DEFAULT_PROJECT_ID);
    flags.str_flag("base-url", DEFAULT_BASE_URL);
    flags.str_flag("job-name", DEFAULT_RELEASE_JOB);
    flags.bool_flag("no-verify", false);
    flags.str_flag(
        "job-token",
        &std::env::var("CI_JOB_TOKEN").unwrap_or_default(),
    );
    flags.describe("version", "upstream tag, e.g. v0.2.6 (mandatory)");
    flags.describe("artifact", "artifact filename");
    flags.describe("install-dir", "install dir");
    flags.describe("project-id", "upstream project id");
    flags.describe("base-url", "upstream GitLab base URL");
    flags.describe("job-name", "upstream release job name");
    flags.describe("no-verify", "skip post-install version check");
    flags.describe("job-token", "GitLab JOB-TOKEN (default: $CI_JOB_TOKEN)");
    flags
}

/// One string flag's value; the flag set always holds the default.
fn text(flags: &FlagSet, name: &str) -> String {
    flags
        .get(name)
        .map(|value| value.as_str(""))
        .unwrap_or_default()
}

/// Parses one subcommand's flags into a recipe, reporting refusals the
/// way Go's `flag.ContinueOnError` does: the error line and the usage
/// block on stderr (usage alone for `-h`), then the error itself so
/// `main` adds its `Error:` line.
fn parse_flags(name: &str, args: &[String]) -> Result<CIRecipe, String> {
    let mut flags = build_flags();
    if let Err(error) = flags.parse(args) {
        report_parse_error(&error, &flags.render_usage(&format!("ci-recipe {name}")));
        return Err(error.to_string());
    }
    let recipe = CIRecipe {
        version: text(&flags, "version"),
        artifact: text(&flags, "artifact"),
        install_dir: text(&flags, "install-dir"),
        project_id: text(&flags, "project-id"),
        base_url: text(&flags, "base-url"),
        release_job: text(&flags, "job-name"),
        job_token: text(&flags, "job-token"),
        expect_verify: !flags
            .get("no-verify")
            .is_some_and(|value| value.as_bool(false)),
    };
    if recipe.version.is_empty() {
        return Err("--version is required (e.g. --version v0.2.6)".to_string());
    }
    Ok(recipe)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `&["a", "b"]` -> `["a".to_string(), "b".to_string()]`.
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn the_usage_block_is_byte_identical_to_go() {
        assert_eq!(USAGE.len(), 1382);
        assert!(USAGE.starts_with("ci-recipe — render / download the artifact recipe"));
        assert!(USAGE.ends_with("--install-dir /tmp\n"));
        assert!(!USAGE.contains('"'));
    }

    #[test]
    fn no_arguments_print_the_usage_block() {
        assert_eq!(ci_recipe(&[]).unwrap(), 0);
    }

    #[test]
    fn help_prints_the_usage_block_and_ignores_extra_arguments() {
        for help in ["-h", "--help", "help"] {
            assert_eq!(ci_recipe(&args(&[help, "extra"])).unwrap(), 0);
        }
    }

    #[test]
    fn list_prints_every_supported_artifact() {
        assert_eq!(ci_recipe(&args(&["list"])).unwrap(), 0);
        // `list` parses no flags at all.
        assert_eq!(
            ci_recipe(&args(&["list", "--version", "v1.2.3"])).unwrap(),
            0
        );
    }

    #[test]
    fn an_unknown_subcommand_names_the_help_command() {
        let error = ci_recipe(&args(&["badsub"])).unwrap_err();
        assert_eq!(
            error,
            "ci-recipe: unknown subcommand \"badsub\" (see `ci-recipe help`)"
        );
    }

    #[test]
    fn url_requires_a_version() {
        let error = url_command(&[]).unwrap_err();
        assert_eq!(error, "--version is required (e.g. --version v0.2.6)");
        // Go's flag package stops at the first positional argument.
        let error = url_command(&args(&["positional", "--version", "v1.2.3"])).unwrap_err();
        assert_eq!(error, "--version is required (e.g. --version v0.2.6)");
    }

    #[test]
    fn url_rejects_a_non_semver_version() {
        let error = url_command(&args(&["--version", "bad"])).unwrap_err();
        assert_eq!(error, "ci-recipe: Version \"bad\" must match vX.Y.Z");
    }

    #[test]
    fn url_rejects_an_unknown_artifact() {
        let error = url_command(&args(&["--version", "v1.2.3", "--artifact", "nope"])).unwrap_err();
        assert!(
            error.starts_with("ci-recipe: Artifact \"nope\" is not one of ["),
            "{error}"
        );
    }

    #[test]
    fn url_and_script_render_their_output() {
        assert_eq!(url_command(&args(&["--version", "v0.2.6"])).unwrap(), 0);
        assert_eq!(
            script_command(&args(&["--version", "v0.2.6", "--no-verify"])).unwrap(),
            0
        );
    }

    #[test]
    fn download_refuses_a_missing_token_before_touching_the_network() {
        let error = download_command(&args(&[
            "--version",
            "v1.2.3",
            "--artifact",
            "nope",
            "--job-token",
            "",
        ]))
        .unwrap_err();
        assert_eq!(error, "--job-token missing and $CI_JOB_TOKEN unset");
        // The token check comes after the version check, before `Validate`.
        let error = download_command(&args(&["--job-token", ""])).unwrap_err();
        assert_eq!(error, "--version is required (e.g. --version v0.2.6)");
    }

    #[test]
    fn parse_errors_carry_the_go_flag_wording() {
        let error = parse_flags("url", &args(&["--bogus"])).unwrap_err();
        assert_eq!(error, "flag provided but not defined: -bogus");
        let error = parse_flags("url", &args(&["--version"])).unwrap_err();
        assert_eq!(error, "flag needs an argument: -version");
        let error = parse_flags("url", &args(&["-h"])).unwrap_err();
        assert_eq!(error, "flag: help requested");
    }

    #[test]
    fn flags_stop_at_the_first_positional_and_at_the_terminator() {
        let recipe = parse_flags(
            "url",
            &args(&["--version", "v0.2.6", "--", "--job-name", "ignored"]),
        )
        .unwrap();
        assert_eq!(recipe.release_job, DEFAULT_RELEASE_JOB);
        let recipe = parse_flags(
            "url",
            &args(&["extra", "--version", "v0.2.6", "--job-name", "x"]),
        )
        .unwrap_err();
        assert_eq!(recipe, "--version is required (e.g. --version v0.2.6)");
    }

    #[test]
    fn the_token_default_comes_from_the_environment() {
        let recipe = parse_flags("url", &args(&["--version", "v0.2.6"])).unwrap();
        assert_eq!(
            recipe.job_token,
            std::env::var("CI_JOB_TOKEN").unwrap_or_default()
        );
    }
}
