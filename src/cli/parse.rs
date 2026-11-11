// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The parsing half of the flag set: one argv scan, Go's rules.

use super::{parse_go_bool, FlagKind, FlagSet, FlagValue, ParseError};
use crate::goparse;

/// One token of a flag stream.
enum Token {
    /// A non-flag argument: parsing stops here.
    Positional,
    /// The `--` terminator.
    End,
    /// A flag, with the inline value when it carried one.
    Flag {
        /// The name as written, dashes stripped.
        name: String,
        /// The `=value` half, when present.
        inline: Option<String>,
    },
}

impl FlagSet {
    /// Parses `argv`, filling in registered flags and collecting positionals.
    ///
    /// The rules are Go's `flag` rules: parsing stops at the first
    /// positional argument, `--` terminates the flags, `-name=value` carries
    /// an inline value, and `-h` / `-help` answer with [`ParseError::Help`]
    /// when neither of them is registered.
    pub fn parse(&mut self, argv: &[String]) -> Result<(), ParseError> {
        let mut positionals: Vec<String> = Vec::new();
        let mut index = 0;
        while index < argv.len() {
            match split_token(&argv[index])? {
                Token::Positional => {
                    positionals.extend_from_slice(&argv[index..]);
                    break;
                }
                Token::End => {
                    positionals.extend_from_slice(&argv[index + 1..]);
                    break;
                }
                Token::Flag { name, inline } => {
                    index = self.apply(argv, index, &name, inline)?;
                }
            }
        }
        self.args = positionals;
        Ok(())
    }

    /// Applies one `-name[=value]` token and returns the index to read next.
    fn apply(
        &mut self,
        argv: &[String],
        index: usize,
        raw_name: &str,
        inline: Option<String>,
    ) -> Result<usize, ParseError> {
        let name = self.resolve(raw_name);
        let Some(kind) = self.kinds.get(&name).copied() else {
            return Err(unknown_flag(raw_name));
        };
        self.provided.insert(name.clone());
        if kind == FlagKind::Bool {
            // A boolean flag never consumes the next argument.
            let raw = inline.unwrap_or_else(|| "true".to_string());
            let value = self.coerce(raw_name, kind, raw)?;
            self.values.insert(name, value);
            return Ok(index + 1);
        }
        let (raw, next) = match inline {
            Some(value) => (value, index + 1),
            None => {
                let value =
                    argv.get(index + 1)
                        .cloned()
                        .ok_or_else(|| ParseError::MissingValue {
                            name: raw_name.to_string(),
                        })?;
                (value, index + 2)
            }
        };
        // Go's list flags ignore an empty value and keep the old one.
        if kind == FlagKind::List && raw.is_empty() {
            return Ok(next);
        }
        let value = self.coerce(raw_name, kind, raw)?;
        self.values.insert(name, value);
        Ok(next)
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
            // Go's durationValue reports every refusal as `parse error`.
            FlagKind::Duration => goparse::parse_duration(&raw)
                .map(FlagValue::Duration)
                .ok_or_else(|| ParseError::InvalidValue {
                    name: name.to_string(),
                    value: raw,
                    reason: "parse error".to_string(),
                }),
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

/// Classifies one argument the way Go's `flag` package does.
fn split_token(arg: &str) -> Result<Token, ParseError> {
    // Go stops as soon as a token is not a flag: `len(s) < 2 || s[0] != '-'`.
    if arg.len() < 2 || !arg.starts_with('-') {
        return Ok(Token::Positional);
    }
    if arg == "--" {
        return Ok(Token::End);
    }
    let dashes = if arg.starts_with("--") { 2 } else { 1 };
    let body = &arg[dashes..];
    if body.starts_with('-') || body.starts_with('=') {
        return Err(ParseError::BadSyntax {
            arg: arg.to_string(),
        });
    }
    Ok(match body.split_once('=') {
        Some((name, value)) => Token::Flag {
            name: name.to_string(),
            inline: Some(value.to_string()),
        },
        None => Token::Flag {
            name: body.to_string(),
            inline: None,
        },
    })
}

/// Go's answer for an unregistered flag: `-h` asks for help, the rest is an
/// error.
fn unknown_flag(raw_name: &str) -> ParseError {
    if raw_name == "h" || raw_name == "help" {
        return ParseError::Help;
    }
    ParseError::UnknownFlag {
        name: raw_name.to_string(),
    }
}
