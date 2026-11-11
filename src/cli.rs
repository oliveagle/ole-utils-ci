// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! A small `flag`-style argument parser.
//!
//! The Go original leans on Go's `flag` package, so this parser reproduces
//! that behaviour rather than inventing a new one:
//!
//! * `-name value`, `-name=value`, `--name value` and `--name=value` all work.
//! * Boolean flags may appear without a value (`-v`) or with one
//!   (`-v=false`).
//! * A flag may have aliases, so `-v` and `--verbose` set the same value.
//! * Parsing stops at the first non-flag argument; the rest are positional.
//! * An unknown flag is an error, not a silently ignored argument.
//!
//! `clap` would have been the obvious dependency, but it cannot be compiled by
//! [blaze](https://github.com/indexable-inc/blaze) (it reaches
//! `std::io::stderr`'s writable static), so the parser is hand-written.

use std::collections::{BTreeMap, BTreeSet};

use crate::goparse;

/// The type of a registered flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlagKind {
    /// Presence/absence flag.
    Bool,
    /// Free-form string flag.
    Str,
    /// Numeric flag.
    Int,
    /// Duration flag, stored as nanoseconds.
    Duration,
    /// Comma-separated list flag, stored as one string.
    List,
    /// Floating-point flag.
    Float,
    /// Repeatable value flag: every occurrence appends its parts.
    Value,
}

/// A parsed flag value.
#[derive(Debug, Clone, PartialEq)]
pub enum FlagValue {
    /// Boolean flag.
    Bool(bool),
    /// String flag.
    Str(String),
    /// Integer flag.
    Int(i64),
    /// Duration flag in nanoseconds.
    Duration(i64),
    /// Comma-separated list flag, split on commas.
    List(Vec<String>),
    /// Floating-point flag.
    Float(f64),
    /// Repeatable value flag: the parts accumulated over every occurrence.
    Values(Vec<String>),
}

/// Error produced while parsing the command line.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// A flag that was never registered.
    #[error("flag provided but not defined: -{name}")]
    UnknownFlag {
        /// The offending flag name, without dashes.
        name: String,
    },
    /// Go's answer to `-h`, `-help` or `--help` when neither is registered.
    #[error("flag: help requested")]
    Help,
    /// A token whose spelling is not a legal flag at all (`---x`, `--=x`).
    #[error("bad flag syntax: {arg}")]
    BadSyntax {
        /// The offending argument, dashes and all.
        arg: String,
    },
    /// A value that could not be parsed as a boolean flag's type.
    #[error("invalid boolean value {value:?} for -{name}: {reason}")]
    InvalidBoolValue {
        /// The offending flag name, without dashes.
        name: String,
        /// The raw value the caller supplied.
        value: String,
        /// Why the value was rejected.
        reason: String,
    },
    /// A flag that needs a value but was given none.
    #[error("flag needs an argument: -{name}")]
    MissingValue {
        /// The offending flag name, without dashes.
        name: String,
    },
    /// A value that could not be parsed as the flag's type.
    #[error("invalid value {value:?} for flag -{name}: {reason}")]
    InvalidValue {
        /// The offending flag name, without dashes.
        name: String,
        /// The raw value the caller supplied.
        value: String,
        /// Why the value was rejected.
        reason: String,
    },
}

/// A registry of flags plus the values parsed for them.
#[derive(Debug, Default)]
pub struct FlagSet {
    kinds: BTreeMap<String, FlagKind>,
    values: BTreeMap<String, FlagValue>,
    provided: BTreeSet<String>,
    aliases: BTreeMap<String, String>,
    /// The help text attached to a flag by [`FlagSet::describe`].
    usages: BTreeMap<String, String>,
    /// The registered default of each flag, spelled the way Go's `flag`
    /// records `DefValue`.
    defs: BTreeMap<String, String>,
    /// The type name each flag prints in the usage block, when it is not the
    /// name of the flag's kind.
    types: BTreeMap<String, String>,
    /// Positional arguments left after the flags.
    pub args: Vec<String>,
}

impl FlagSet {
    /// Creates an empty flag set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a boolean flag with its default value.
    pub fn bool_flag(&mut self, name: &str, default: bool) -> &mut Self {
        self.kinds.insert(name.to_string(), FlagKind::Bool);
        self.values
            .insert(name.to_string(), FlagValue::Bool(default));
        self.defs.insert(name.to_string(), default.to_string());
        self
    }

    /// Registers a string flag with its default value.
    pub fn str_flag(&mut self, name: &str, default: &str) -> &mut Self {
        self.kinds.insert(name.to_string(), FlagKind::Str);
        self.values
            .insert(name.to_string(), FlagValue::Str(default.to_string()));
        self.defs.insert(name.to_string(), default.to_string());
        self
    }

    /// Registers an integer flag with its default value.
    pub fn int_flag(&mut self, name: &str, default: i64) -> &mut Self {
        self.kinds.insert(name.to_string(), FlagKind::Int);
        self.values
            .insert(name.to_string(), FlagValue::Int(default));
        self.defs.insert(name.to_string(), default.to_string());
        self
    }

    /// Registers a duration flag (`5s`, `10m`, `1h30m`) with its default,
    /// spelled the way Go's `time.Duration.String` spells it.
    pub fn duration_flag(&mut self, name: &str, default: &str) -> &mut Self {
        let nanos = goparse::parse_duration(default).unwrap_or(0);
        self.kinds.insert(name.to_string(), FlagKind::Duration);
        self.values
            .insert(name.to_string(), FlagValue::Duration(nanos));
        self.defs.insert(name.to_string(), duration_string(nanos));
        self
    }

    /// Registers a comma-separated list flag with its default value.
    pub fn list_flag(&mut self, name: &str, default: &[String]) -> &mut Self {
        self.kinds.insert(name.to_string(), FlagKind::List);
        self.values
            .insert(name.to_string(), FlagValue::List(default.to_vec()));
        self.defs.insert(name.to_string(), default.join(","));
        self
    }

    /// Registers a floating-point flag with its default value.
    pub fn float_flag(&mut self, name: &str, default: f64) -> &mut Self {
        self.kinds.insert(name.to_string(), FlagKind::Float);
        self.values
            .insert(name.to_string(), FlagValue::Float(default));
        self.defs.insert(name.to_string(), format_go_float(default));
        self
    }

    /// Registers a repeatable value flag (`fs.Var` over a `[]string`).
    ///
    /// Every occurrence appends the comma-separated parts, trimming the
    /// whitespace around each and dropping the empty ones — Go's
    /// `stringListFlag.Set` in `web_lint_cmd.go`.
    pub fn value_flag(&mut self, name: &str) -> &mut Self {
        self.kinds.insert(name.to_string(), FlagKind::Value);
        self.values
            .insert(name.to_string(), FlagValue::Values(Vec::new()));
        self.defs.insert(name.to_string(), String::new());
        self
    }

    /// Overrides the type name the usage block prints for `name`.
    ///
    /// Go's `flag.UnquoteUsage` lifts a backquoted word out of the usage
    /// text into the type position; the commands that spell a type that way
    /// register it here instead.
    pub fn typ(&mut self, name: &str, type_name: &str) -> &mut Self {
        self.types.insert(name.to_string(), type_name.to_string());
        self
    }

    /// Attaches the help text Go's `flag` would print for `name` in its
    /// `PrintDefaults` block.
    pub fn describe(&mut self, name: &str, usage: &str) -> &mut Self {
        self.usages.insert(name.to_string(), usage.to_string());
        self
    }

    /// Registers an alias for an existing flag, so `-v` and `--verbose` set
    /// the same value.
    pub fn alias(&mut self, from: &str, to: &str) -> &mut Self {
        self.aliases.insert(from.to_string(), to.to_string());
        self
    }

    /// Resolves a flag name through the alias table.
    fn resolve(&self, name: &str) -> String {
        match self.aliases.get(name) {
            Some(target) => target.clone(),
            None => name.to_string(),
        }
    }

    /// Looks up a parsed value.
    pub fn get(&self, name: &str) -> Option<&FlagValue> {
        self.values.get(&self.resolve(name))
    }

    /// Whether the named flag was present on the command line.
    pub fn was_provided(&self, name: &str) -> bool {
        self.provided.contains(&self.resolve(name))
    }
}

mod local;
mod parse;
mod usage;
mod values;

pub use local::report_parse_error;
pub use usage::GLOBAL_USAGE;
pub use values::{format_go_float, parse_go_bool};

use values::duration_string;

#[cfg(test)]
mod tests;
