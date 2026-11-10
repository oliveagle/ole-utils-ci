#!/usr/bin/env bash
# Copyright 2026 devine_utils contributors.
# Licensed under the Apache License, Version 2.0.
# SPDX-License-Identifier: Apache-2.0
#
# Thin wrapper around `blaze`.
#
# The blaze daemon sandboxes every compilation inside a user namespace. Many
# container images block `unshare(CLONE_NEWUSER)`, and the daemon then fails
# with `/proc/self/setgroups: Permission denied`. The wrapper retries with
# `BLAZE_SANDBOX=off`, which keeps the daemon's incremental cache but drops the
# namespace isolation.

set -euo pipefail

BLAZE_BIN="${BLAZE_BIN:-blaze}"

if ! command -v "${BLAZE_BIN}" >/dev/null 2>&1; then
  echo "blaze is not installed; see https://github.com/indexable-inc/blaze" >&2
  exit 127
fi

# The sandbox setting is read when the daemon starts, so an already-running
# daemon has to be restarted before the new value takes effect.
if [[ "${BLAZE_SANDBOX:-off}" == "off" ]]; then
  "${BLAZE_BIN}" stop >/dev/null 2>&1 || true
  export BLAZE_SANDBOX=off
fi

exec "${BLAZE_BIN}" "$@"
