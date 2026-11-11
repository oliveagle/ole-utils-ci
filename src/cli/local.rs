// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The usage block of a *private* flag set.
//!
//! The global flags print the hand-written [`GLOBAL_USAGE`](super::GLOBAL_USAGE)
//! block, but the commands that own their own flag set (`ci-recipe`,
//! `upgrade`, `logs`, `pipeline-run`, ...) must print whatever Go's
//! `flag.PrintDefaults` would print for the flags they registered: sorted
//! names, the `-name type` header line, the four-space continuation indent,
//! the one-letter boolean exception, and the `(default ...)` suffix rules.

use super::{FlagKind, FlagSet, ParseError};

impl FlagSet {
    /// Renders `Usage of <set_name>:` plus Go's `PrintDefaults` block.
    ///
    /// The output is byte-identical to the Go original for every flag the
    /// local sets register: string, int, duration and boolean flags with
    /// single-line usage text.
    pub fn render_usage(&self, set_name: &str) -> String {
        let mut out = format!("Usage of {set_name}:\n");
        for (name, kind) in &self.kinds {
            let mut line = format!("  -{name}");
            let type_name = match kind {
                FlagKind::Bool => "",
                FlagKind::Str => "string",
                FlagKind::Int => "int",
                FlagKind::Duration => "duration",
                FlagKind::List => "strings",
            };
            if !type_name.is_empty() {
                line.push(' ');
                line.push_str(type_name);
            }
            // Go puts a one-letter boolean's usage on the same line: the
            // buffer holds exactly `  -f` (four bytes) at this point.
            if line.len() <= 4 {
                line.push('\t');
            } else {
                line.push_str("\n    \t");
            }
            let usage = self.usages.get(name).map(String::as_str).unwrap_or("");
            line.push_str(&usage.replace('\n', "\n    \t"));
            let def = self.defs.get(name).map(String::as_str).unwrap_or("");
            if !is_zero_default(kind, def) {
                if *kind == FlagKind::Str {
                    line.push_str(&format!(" (default {def:?})"));
                } else {
                    line.push_str(&format!(" (default {def})"));
                }
            }
            out.push_str(&line);
            out.push('\n');
        }
        out
    }
}

/// Whether `def` is the zero value of `kind`, in which case Go omits the
/// `(default ...)` suffix.
fn is_zero_default(kind: &FlagKind, def: &str) -> bool {
    match kind {
        FlagKind::Bool => def == "false",
        FlagKind::Str | FlagKind::List => def.is_empty(),
        FlagKind::Int => def == "0",
        FlagKind::Duration => def == "0s",
    }
}

/// Reports a private flag-set failure the way Go's `flag` does: every
/// refusal prints the error line and then the usage block on stderr, while
/// `-h` prints the usage block alone. The caller still returns the error so
/// `main` can add its own `Error:` line — except for `-h`, where the caller
/// exits 0 the way `runLogs` does.
pub fn report_parse_error(err: &ParseError, usage: &str) {
    match err {
        ParseError::Help => eprint!("{usage}"),
        other => {
            eprintln!("{other}");
            eprint!("{usage}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ci_recipe_url_flags() -> FlagSet {
        let mut fs = FlagSet::new();
        fs.str_flag("version", "");
        fs.str_flag("artifact", "devine-utils-ci-linux-amd64.gz");
        fs.str_flag("install-dir", "bin");
        fs.str_flag("project-id", "158442");
        fs.str_flag("base-url", "http://git.dev.sh.ctripcorp.com");
        fs.str_flag("job-name", "release");
        fs.bool_flag("no-verify", false);
        fs.str_flag("job-token", "");
        fs.describe("version", "upstream tag, e.g. v0.2.6 (mandatory)");
        fs.describe("artifact", "artifact filename");
        fs.describe("install-dir", "install dir");
        fs.describe("project-id", "upstream project id");
        fs.describe("base-url", "upstream GitLab base URL");
        fs.describe("job-name", "upstream release job name");
        fs.describe("no-verify", "skip post-install version check");
        fs.describe("job-token", "GitLab JOB-TOKEN (default: $CI_JOB_TOKEN)");
        fs
    }

    #[test]
    fn renders_the_go_usage_block_for_ci_recipe_url() {
        let expected = [
            "Usage of ci-recipe url:",
            "  -artifact string",
            "    \tartifact filename (default \"devine-utils-ci-linux-amd64.gz\")",
            "  -base-url string",
            "    \tupstream GitLab base URL (default \"http://git.dev.sh.ctripcorp.com\")",
            "  -install-dir string",
            "    \tinstall dir (default \"bin\")",
            "  -job-name string",
            "    \tupstream release job name (default \"release\")",
            "  -job-token string",
            "    \tGitLab JOB-TOKEN (default: $CI_JOB_TOKEN)",
            "  -no-verify",
            "    \tskip post-install version check",
            "  -project-id string",
            "    \tupstream project id (default \"158442\")",
            "  -version string",
            "    \tupstream tag, e.g. v0.2.6 (mandatory)",
        ]
        .join("\n");
        let expected = format!("{expected}\n");
        assert_eq!(
            ci_recipe_url_flags().render_usage("ci-recipe url"),
            expected
        );
    }

    #[test]
    fn a_one_letter_boolean_keeps_its_usage_on_the_same_line() {
        let mut fs = FlagSet::new();
        fs.bool_flag("f", false);
        fs.describe("f", "持续 tail 模式 (简写)");
        assert_eq!(
            fs.render_usage("logs"),
            "Usage of logs:\n  -f\t持续 tail 模式 (简写)\n"
        );
    }

    #[test]
    fn non_zero_defaults_appear_in_every_flavour() {
        let mut fs = FlagSet::new();
        fs.int_flag("app-id", 100075290);
        fs.duration_flag("interval", "5s");
        fs.bool_flag("follow", false);
        fs.bool_flag("dry-run", true);
        fs.describe("app-id", "应用 ID (默认: 100075290 = devine-fe)");
        fs.describe("interval", "tail 轮询间隔 (如 5s, 10s)");
        fs.describe("follow", "持续 tail 模式 (轮询)");
        fs.describe("dry-run", "do nothing");
        let usage = fs.render_usage("logs");
        assert!(usage.contains("(default 100075290)"));
        assert!(usage.contains("(default 5s)"));
        assert!(usage.contains("(default true)"));
        assert!(!usage.contains("(default false)"));
    }
}
