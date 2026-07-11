#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -lt 3 ]]; then
  echo "core-gate-linux-supervisor: expected stdout stderr executable [args...]" >&2
  exit 64
fi

stdout_path="$1"
stderr_path="$2"
shift 2

exec >>"${stdout_path}" 2>>"${stderr_path}"
exec "$@"
