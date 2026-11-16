// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The short, human-facing help for the focused Rhai gate.

/// Printed after the generated flag block for `-h`.
pub const FRIENDLY_USAGE: &str = r#"
rhai-check — syntax, line/branch coverage and complexity for Rhai sources.

Source selection
  --src DIR                 source root searched for *.rhai (default .)
  --max-files N             maximum files checked (default 200)
  --max-file-size BYTES     maximum source size (default 1 MiB)

Coverage cases
  --cases PATH              JSON file or directory of test calls
  --timeout DURATION        warning cap recorded per case (default 5s)
  --max-operations N        Rhai operation budget per call (default 2,000,000)

Budgets
  --line-threshold N        line coverage floor in percent (default 80)
  --branch-threshold N      branch coverage floor in percent (default 80)
  --complexity-warn N       complexity budget (default 15)
  --max-complexity N        complexity hard cap (default 20)

Case files are JSON arrays, or objects with a "cases" array. One call is:
  {"name":"true path", "file":"kernel.rhai", "function":"hook",
   "args":[{}], "expected":{"repair":"none"}}

The command is a gate: any syntax error, budget breach, test fault or
coverage shortfall exits 1. --output accepts text, json, or toon.
"#;
