// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The short, human-facing help for the focused MoonBit gate.

/// Printed after the generated flag block for `-h`.
pub const FRIENDLY_USAGE: &str = r#"
moonbit-check — check, format, test, line coverage and complexity for MoonBit.

Source selection
  --src DIR                 source root searched for *.mbt (default .)
                            a moon.mod must exist at or above this directory
  --max-files N             maximum files checked (default 200)
  --max-file-size BYTES     maximum source size (default 1 MiB)

Checks
  --check-format=true|false run `moon fmt --check` (default true)
  --run-tests=true|false    run `moon test` (default true)
  --coverage=true|false     instrument tests and read Cobertura line coverage
                            (default true)
  --test-filter GLOB        pass `--filter GLOB` to `moon test`
  --deny-warnings=true|false pass `--deny-warn` to `moon check` (default true)
  --timeout DURATION        per-command wall-clock budget (default 5m)

Budgets
  --line-threshold N        line coverage floor in percent (default 80)
  --complexity-warn N       complexity budget (default 15)
  --max-complexity N        complexity hard cap (default 20)

The MoonBit toolchain reports line coverage, not branch outcomes; the gate
therefore never pretends to know branch coverage. A missing `moon.mod` is a
usage error, not a silently empty run.

The command is a gate: any compiler diagnostic, formatter drift, test fault,
line-coverage shortfall or complexity breach exits 1. --output accepts text,
json, or toon.
"#;
