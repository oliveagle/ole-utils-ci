// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The friendly usage block `web-lint -h` prints on stdout.
//!
//! Go prints two blocks for `-h`: the `flag` package's own `Usage of
//! web-lint:` on stderr and this hand-written one on stdout. Both are
//! reproduced verbatim.

/// The block `printWebLintUsage` writes to stdout.
pub const FRIENDLY_USAGE: &str = r#"web-lint — syntax-level lint for HTML / CSS / JavaScript / TypeScript

Usage:
  devine-utils-ci web-lint [flags]

Flags:
  --project-root string        project root; also the default security root
  --path string                file or directory to lint
  --content string             inline source to lint (requires --language)
  --language string            html|css|js|ts|tsx|jsx
  --root string                security root; resolved paths must stay inside
  --ts-engine string           auto|builtin|external (default auto)
  --tsconfig string            tsconfig.json path for external tsc
  --max-files int              cap on files linted (default 200)
  --max-file-size int          per-file byte cap (default 1048576)
  --external-timeout-sec int   timeout for external tsc (default 60)
  --ignore string              comma-separated path substrings to skip
  --format string              text|json|toon (default text)
  --fail-on string             error|warning|none (default error)

Exit codes:
  0  no findings at or above --fail-on
  1  findings at or above --fail-on
  2  usage error / engine failure
"#;
