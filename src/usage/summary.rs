// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The short help block: what a bare invocation prints.
//!
//! The full block is several hundred lines, and a caller that only wants to
//! remember the shape of the command line pays for all of it in context. This
//! one names the groups, the commands people actually type, and where the
//! rest lives.

pub(super) const SUMMARY: &str = r#"devine-utils-ci — Unified CI Quality Gate Tool

Usage:
  devine-utils-ci <group> <command> [flags]

Groups:
  golang   Go toolchain, mutation testing and the Go-specific gates.
  rust     The Rust gates: fmt, clippy, test, build, doc, audit, policies.
  tengo    The Tengo tool gates: unit tests, coverage and mutation testing.
  proto    The protobuf gates: buf lint, breaking changes and code generation.
  web      The web gate: HTML, CSS, JavaScript and TypeScript.
  python   The Python gates: ruff, mypy, pytest, and the source gates.
  mojo     The Mojo gates: format, build, test, package, doc and safety.
  common   The language-agnostic gates every project runs.

Everyday:
  devine-utils-ci all                    every gate this repository runs
  devine-utils-ci rust all               every Rust gate, in order
  devine-utils-ci hook install           install pre-commit, commit-msg and pre-push
  devine-utils-ci <group> <command> --help   the flags of one command

  Nothing runs until you name a command. Run `devine-utils-ci help` for the
  full command list and the global flags.
"#;
