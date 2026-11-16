// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Language-scoped subcommands.
//!
//! The flat command surface prefixes every language-specific command with
//! the language: `tengo-test`, `rust-clippy`, `go-pin`. That keeps the
//! names unique, but the reader has to know the prefix before the
//! alphabetical list is useful. This module groups the commands by the
//! language they gate and exposes every group twice:
//!
//! * as a subcommand of the main binary — `devine-utils-ci rust clippy`;
//! * as a standalone binary — `ole-utils-rust clippy`.
//!
//! Both spellings run the same code: a group is a view over the flat
//! surface, never a second implementation of it.

mod common;
mod registry;
#[cfg(test)]
mod tests;

pub use registry::LANGUAGES;

use crate::dispatch;
use crate::usage;

/// One language group.
pub struct Language {
    /// The name used on the command line.
    pub name: &'static str,
    /// The language's own name.
    pub title: &'static str,
    /// One line describing what the group gates.
    pub summary: &'static str,
    /// The group's subcommands: `(subcommand, flat command, description)`.
    ///
    /// The flat command name is accepted as an alias too, so
    /// `rust rust-clippy` runs the same gate as `rust clippy`.
    pub commands: &'static [(&'static str, &'static str, &'static str)],
    /// The subcommands the group runs when it is called without one.
    ///
    /// This is the "run every gate of this language" path.
    pub aggregate: &'static [&'static str],
}

/// Looks a group up by name.
pub fn find(name: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|lang| lang.name == name)
}

/// The standalone binary that runs only this group.
pub fn binary_name(lang: &Language) -> String {
    format!("ole-utils-{}", lang.name)
}

impl Language {
    /// Resolves a subcommand to the flat command it forwards to.
    pub fn resolve(&self, sub: &str) -> Option<&'static str> {
        self.commands
            .iter()
            .find(|(alias, command, _)| *alias == sub || *command == sub)
            .map(|(_, command, _)| *command)
    }

    /// The name a flat command is known by inside the group.
    ///
    /// The subcommand that forwards to it is the label the user typed.
    pub fn label(&self, command: &'static str) -> &'static str {
        self.commands
            .iter()
            .find(|(_, flat, _)| *flat == command)
            .map(|(alias, _, _)| *alias)
            .unwrap_or(command)
    }

    /// The flat commands the group's `all` runs, in order.
    pub fn aggregate_commands(&self) -> &'static [&'static str] {
        self.aggregate
    }
}

/// The subcommand that runs every gate of a group.
pub const ALL: &str = "all";

/// Runs a language group.
///
/// `args` is what follows the group name: the subcommand first, then the
/// flags of the command it forwards to.
///
/// A group without a subcommand runs nothing: it prints the commands it
/// has and exits. Only an explicit command starts a gate, so a mistyped
/// group name can never run the wrong thing.
pub fn run(lang: &Language, args: &[String]) -> Result<i32, String> {
    match subcommand(args) {
        Some("help") => {
            usage::print_language(lang);
            Ok(0)
        }
        Some(ALL) => aggregate(lang, &args[1..]),
        Some(sub) => match lang.resolve(sub) {
            Some(command) => dispatch::dispatch_flat(command, &args[1..]),
            None => {
                println!("Unknown {} command: {sub}\n", lang.name);
                usage::print_language(lang);
                Ok(1)
            }
        },
        None => {
            if wants_help(args) {
                usage::print_language(lang);
                return Ok(0);
            }
            usage::print_language(lang);
            Ok(1)
        }
    }
}

/// The subcommand in `args`, if there is one.
///
/// A leading flag means the group was called without a subcommand: the
/// flags then belong to the aggregate run.
fn subcommand(args: &[String]) -> Option<&str> {
    match args.first() {
        Some(first) if !first.starts_with('-') => Some(first.as_str()),
        _ => None,
    }
}

/// Whether the caller asked for help.
fn wants_help(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--help" || arg == "-h")
}

/// Runs every gate of the group, in order.
///
/// The first failure stops the run, exactly like the `all` command does.
fn aggregate(lang: &Language, args: &[String]) -> Result<i32, String> {
    // A group whose aggregate is the command that bears its name (rust, python, mojo) already runs
    // its gates and prints their verdicts. Wrapping it would draw a header around a run that
    // printed one per gate and then say the same thing twice: "── mojo ──", six gates,
    // "All Mojo gates passed", "mojo passed", "All Mojo gates passed".
    if let [only] = lang.aggregate {
        if *only == lang.name {
            return dispatch::dispatch_flat(only, args);
        }
    }
    let mut failures: Vec<&str> = Vec::new();
    for command in lang.aggregate {
        let label = lang.label(command);
        println!("── {label} ──");
        match dispatch::dispatch_flat(command, args) {
            Ok(0) => println!("✅ {label} passed"),
            Ok(code) => {
                failures.push(label);
                println!("✗ {label} failed (exit {code})");
                break;
            }
            Err(e) => {
                failures.push(label);
                println!("✗ {label} failed: {e}");
                break;
            }
        }
    }
    if failures.is_empty() {
        println!("✅ All {} gates passed", lang.title);
        Ok(0)
    } else {
        Err(format!(
            "{} gate(s) failed: {}",
            lang.title,
            failures.join(", ")
        ))
    }
}
