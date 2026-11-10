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
    /// Comma-separated list flag, stored as one string.
    List,
}

/// A parsed flag value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagValue {
    /// Boolean flag.
    Bool(bool),
    /// String flag.
    Str(String),
    /// Integer flag.
    Int(i64),
    /// Comma-separated list flag, split on commas.
    List(Vec<String>),
}

impl FlagValue {
    /// Returns the boolean payload, or `default` for other kinds.
    pub fn as_bool(&self, default: bool) -> bool {
        match self {
            FlagValue::Bool(v) => *v,
            _ => default,
        }
    }

    /// Returns the string payload, or `default` for other kinds.
    pub fn as_str(&self, default: &str) -> String {
        match self {
            FlagValue::Str(v) => v.clone(),
            _ => default.to_string(),
        }
    }

    /// Returns the integer payload, or `default` for other kinds.
    pub fn as_int(&self, default: i64) -> i64 {
        match self {
            FlagValue::Int(v) => *v,
            _ => default,
        }
    }

    /// Returns the list payload, or an empty slice for other kinds.
    pub fn as_list(&self) -> &[String] {
        match self {
            FlagValue::List(v) => v.as_slice(),
            _ => &[],
        }
    }
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
        self
    }

    /// Registers a string flag with its default value.
    pub fn str_flag(&mut self, name: &str, default: &str) -> &mut Self {
        self.kinds.insert(name.to_string(), FlagKind::Str);
        self.values
            .insert(name.to_string(), FlagValue::Str(default.to_string()));
        self
    }

    /// Registers an integer flag with its default value.
    pub fn int_flag(&mut self, name: &str, default: i64) -> &mut Self {
        self.kinds.insert(name.to_string(), FlagKind::Int);
        self.values
            .insert(name.to_string(), FlagValue::Int(default));
        self
    }

    /// Registers a comma-separated list flag with its default value.
    pub fn list_flag(&mut self, name: &str, default: &[String]) -> &mut Self {
        self.kinds.insert(name.to_string(), FlagKind::List);
        self.values
            .insert(name.to_string(), FlagValue::List(default.to_vec()));
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

    /// Parses `argv`, filling in registered flags and collecting positionals.
    ///
    /// The rules are Go's `flag` rules: parsing stops at the first
    /// positional argument, `--` terminates the flags, `-name=value` carries
    /// an inline value, and `-h` / `-help` answer with [`ParseError::Help`]
    /// when neither of them is registered.
    pub fn parse(&mut self, argv: &[String]) -> Result<(), ParseError> {
        let mut positionals: Vec<String> = Vec::new();
        let mut i = 0;
        while i < argv.len() {
            let arg = &argv[i];
            // Go stops as soon as a token is not a flag: `len(s) < 2 || s[0] != '-'`.
            if arg.len() < 2 || !arg.starts_with('-') {
                positionals.extend_from_slice(&argv[i..]);
                break;
            }
            if arg == "--" {
                positionals.extend_from_slice(&argv[i + 1..]);
                break;
            }
            let dashes = if arg.starts_with("--") { 2 } else { 1 };
            let body = &arg[dashes..];
            if body.starts_with('-') || body.starts_with('=') {
                return Err(ParseError::BadSyntax { arg: arg.clone() });
            }
            let (raw_name, inline) = match body.split_once('=') {
                Some((name, value)) => (name.to_string(), Some(value.to_string())),
                None => (body.to_string(), None),
            };
            let name = self.resolve(&raw_name);
            let Some(kind) = self.kinds.get(&name).copied() else {
                if raw_name == "h" || raw_name == "help" {
                    return Err(ParseError::Help);
                }
                return Err(ParseError::UnknownFlag { name: raw_name });
            };
            self.provided.insert(name.clone());
            match kind {
                FlagKind::Bool => {
                    let raw = inline.unwrap_or_else(|| "true".to_string());
                    let value = self.coerce(&raw_name, kind, raw)?;
                    self.values.insert(name, value);
                }
                _ => {
                    let raw = match inline {
                        Some(v) => v,
                        None => {
                            i += 1;
                            argv.get(i)
                                .cloned()
                                .ok_or_else(|| ParseError::MissingValue {
                                    name: raw_name.clone(),
                                })?
                        }
                    };
                    // Go's list flags ignore an empty value and keep the old one.
                    if !(kind == FlagKind::List && raw.is_empty()) {
                        let value = self.coerce(&raw_name, kind, raw)?;
                        self.values.insert(name, value);
                    }
                }
            }
            i += 1;
        }
        self.args = positionals;
        Ok(())
    }

    /// Coerces one raw value into the flag's type, worded like Go's `flag`.
    fn coerce(&self, name: &str, kind: FlagKind, raw: String) -> Result<FlagValue, ParseError> {
        match kind {
            FlagKind::Str => Ok(FlagValue::Str(raw)),
            FlagKind::Int => {
                goparse::parse_int(&raw)
                    .map(FlagValue::Int)
                    .map_err(|e| ParseError::InvalidValue {
                        name: name.to_string(),
                        value: raw,
                        reason: e.as_str().to_string(),
                    })
            }
            // Go splits on commas without trimming: `strings.Split(value, ",")`.
            FlagKind::List => Ok(FlagValue::List(raw.split(',').map(String::from).collect())),
            FlagKind::Bool => match parse_go_bool(&raw) {
                Some(value) => Ok(FlagValue::Bool(value)),
                None => Err(ParseError::InvalidBoolValue {
                    name: name.to_string(),
                    value: raw,
                    reason: "parse error".to_string(),
                }),
            },
        }
    }
}

/// `strconv.ParseBool`: exactly the spellings Go's boolean flags accept.
fn parse_go_bool(raw: &str) -> Option<bool> {
    match raw {
        "1" | "t" | "T" | "TRUE" | "true" | "True" => Some(true),
        "0" | "f" | "F" | "FALSE" | "false" | "False" => Some(false),
        _ => None,
    }
}

mod usage;
pub use usage::GLOBAL_USAGE;

#[cfg(test)]
mod tests;
