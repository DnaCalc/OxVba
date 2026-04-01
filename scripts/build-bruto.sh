#!/usr/bin/env bash
set -euo pipefail

target="${1:-x86_64-unknown-linux-gnu}"

if [[ "$target" == *windows* ]]; then
  artifact="oxvba-bruto.exe"
else
  artifact="oxvba-bruto"
fi

echo "Building oxvba-bruto for ${target}"
cargo build --release -p oxvba-bruto --target "${target}"
echo "Built target/${target}/release/${artifact}"
