// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `hook` command: the checks a git hook runs before a commit or a push.
//!
//! Three questions are worth answering on every developer's machine rather
//! than in CI, because the answer is only cheap to act on before the commit
//! exists:
//!
//! * is this change small enough to be one commit (`pre-commit`);
//! * does the message say what the change is (`commit-msg`);
//! * is this a branch that may be pushed at all (`pre-push`).
//!
//! The command also installs the shims that call it, so a clone becomes
//! guarded with one `devine-utils-ci hook install`.

mod args;
mod commit;
mod install;
mod message;
pub mod policy;
mod push;
mod staged;
#[cfg(test)]
mod tests;

use crate::config::Config;
use crate::output::{Diagnostic, Report};

/// The subcommands, in the order the help block lists them.
pub const SUBCOMMANDS: [(&str, &str); 6] = [
    (
        "install",
        "Install the pre-commit, commit-msg and pre-push hooks.",
    ),
    ("uninstall", "Remove the hooks this command installed."),
    (
        "status",
        "Show which hooks are installed, and whether they are ours.",
    ),
    (
        "pre-commit",
        "Check the staged change before it becomes a commit.",
    ),
    (
        "commit-msg",
        "Check the commit message against the commit convention.",
    ),
    ("pre-push", "Check the refs a push would create or update."),
];

/// Runs the `hook` subcommand.
///
/// Like every other group, a bare `hook` names nothing and runs nothing: it
/// prints its commands and exits non-zero, so a typo can never start a check.
pub fn hook(args: &[String]) -> Result<i32, String> {
    let Some(sub) = args.first().map(String::as_str) else {
        print_help();
        return Ok(1);
    };
    let rest = &args[1..];
    match sub {
        "help" | "-h" | "--help" => {
            print_help();
            Ok(0)
        }
        "install" => install::install(rest),
        "uninstall" => install::uninstall(rest),
        "status" => install::status(rest),
        "pre-commit" => commit::pre_commit(rest),
        "commit-msg" => message::commit_msg(rest),
        "pre-push" => push::pre_push(rest),
        other => {
            println!("Unknown hook subcommand: {other}\n");
            print_help();
            Ok(1)
        }
    }
}

/// Prints the help block of the `hook` command.
pub fn print_help() {
    println!("hook — the commit and push policy checks, built to run from git hooks");
    println!();
    println!("Usage:");
    println!("  devine-utils-ci hook <command> [flags]");
    println!();
    println!("Commands:");
    for (name, description) in SUBCOMMANDS {
        println!("  {name:<14}{description}");
    }
    println!();
    println!("Policy (flag > environment variable > default):");
    println!(
        "  --max-files <n>           Files one commit may touch (default {}, HOOK_MAX_FILES).",
        policy::MAX_FILES_DEFAULT
    );
    println!("  --max-file-bytes <size>   Largest staged file (default 5m, HOOK_MAX_FILE_BYTES).");
    println!("  --protected <a,b>         Branches that only accept merges (default main,master).");
    println!("  --allow-secret-paths      Report .env and *.pem instead of refusing them.");
    println!();
    println!(
        "  A commit may touch at most {} files: past that it is not one idea and the",
        policy::MAX_FILES_DEFAULT
    );
    println!("  review stops reading it as one — split it into commits that each build.");
    println!("  main and master never take a direct push: open a merge request instead.");
    println!("  Every other branch is <type>/<slug>, e.g. feat/hook-checks.");
    println!();
    println!("Install it once per clone:");
    println!("  devine-utils-ci hook install     # pre-commit, commit-msg, pre-push");
    println!("  devine-utils-ci hook status      # what is installed");
}

/// The collected findings of one hook run.
///
/// Each check records a line for the human body and a diagnostic for the
/// structured body; the two never disagree because both come from the same
/// call.
pub struct Verdict {
    /// The structured report handed to `--output json` and `--output toon`.
    pub report: Report,
    passed: Vec<String>,
    failed: Vec<String>,
}

impl Verdict {
    /// Starts a verdict for `command`.
    pub fn new(command: &str) -> Self {
        Verdict {
            report: Report::new(command),
            passed: Vec::new(),
            failed: Vec::new(),
        }
    }

    /// Records a check that held.
    pub fn pass(&mut self, line: impl Into<String>) {
        self.passed.push(line.into());
    }

    /// Records a check that did not hold.
    pub fn deny(&mut self, diagnostic: Diagnostic, line: impl Into<String>) {
        self.failed.push(line.into());
        self.report.add(diagnostic);
    }

    /// Attaches the policy numbers to the structured payload.
    pub fn with_extra(mut self, extra: serde_json::Value) -> Self {
        self.report.extra = Some(extra);
        self
    }

    /// Prints the verdict and returns the exit code the hook reports.
    ///
    /// `header` introduces the body (`🔒 pre-commit: 3 staged files`) and
    /// `ok_line` is the sentence a clean run ends with.
    pub fn finish(mut self, cfg: &Config, header: &str, ok_line: &str) -> i32 {
        if self.report.ok {
            self.report.summary = ok_line.to_string();
        } else {
            let count = self.failed.len();
            self.report.summary = format!(
                "{count} problem{} found, the commit or push is refused",
                if count == 1 { "" } else { "s" }
            );
        }
        if cfg.structured() {
            print!("{}", self.report.render(cfg.output_mode()));
        } else {
            println!("{header}");
            for line in &self.passed {
                println!("  ✓ {line}");
            }
            for line in &self.failed {
                println!("  ✗ {line}");
            }
            if self.report.ok {
                println!("✅ {ok_line}");
            } else {
                println!("❌ {}", self.report.summary);
            }
        }
        if self.report.ok {
            return 0;
        }
        let first = self
            .failed
            .first()
            .map(|line| line.lines().next().unwrap_or(line).to_string())
            .unwrap_or_else(|| self.report.summary.clone());
        eprintln!("Error: {first}");
        1
    }
}

/// `count` followed by `word`, pluralised with a plain `s`.
///
/// The hook bodies are read by people in a hurry; "1 files" makes them stop
/// and look for the bug in the wrong place.
pub fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}

/// Parses the global flags after the hook flags were lifted out.
pub(crate) fn parse_globals(args: &[String]) -> Result<(Config, Vec<String>), String> {
    let mut cfg = Config::new();
    let positional = crate::commands::gates::parse(&mut cfg, args)?;
    Ok((cfg, positional))
}

/// The repository root a hook run works on.
pub(crate) fn repo_root(cfg: &Config) -> Result<std::path::PathBuf, String> {
    staged::toplevel(cfg.root())
}
