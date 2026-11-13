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

    /// The flat commands the aggregate runs, in order.
    pub fn aggregate_commands(&self) -> Vec<&'static str> {
        self.aggregate
            .iter()
            .filter_map(|sub| self.resolve(sub))
            .collect()
    }
}

/// Runs a language group.
///
/// `args` is what follows the group name: the subcommand first, then the
/// flags of the command it forwards to.
pub fn run(lang: &Language, args: &[String]) -> Result<i32, String> {
    match subcommand(args) {
        Some("help") => {
            usage::print_language(lang);
            Ok(0)
        }
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
            // The flat surface may already own the group name: `rust` runs
            // every Rust gate on its own and keeps its own error wording.
            // The group only steps in when it has no flat twin.
            if dispatch::has_flat_command(lang.name) {
                return dispatch::dispatch_flat(lang.name, args);
            }
            aggregate(lang, args)
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
    let mut failures: Vec<&str> = Vec::new();
    for sub in lang.aggregate {
        let Some(command) = lang.resolve(sub) else {
            continue;
        };
        println!("── {sub} ──");
        match dispatch::dispatch_flat(command, args) {
            Ok(0) => println!("✅ {sub} passed"),
            Ok(code) => {
                failures.push(sub);
                println!("✗ {sub} failed (exit {code})");
                break;
            }
            Err(e) => {
                failures.push(sub);
                println!("✗ {sub} failed: {e}");
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
