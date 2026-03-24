#!/bin/bash
# Run cargo miri on unsafe-heavy crates.
# Excludes COM crates (Windows FFI is incompatible with miri).
#
# Prerequisites:
#   rustup +nightly component add miri
#
# Usage:
#   bash scripts/run-miri.sh

set -euo pipefail

echo "=== Running miri on oxvba-runtime ==="
cargo +nightly miri test -p oxvba-runtime 2>&1 || {
    echo "FAIL: oxvba-runtime miri"
    exit 1
}

echo "=== Running miri on oxvba-jit ==="
cargo +nightly miri test -p oxvba-jit 2>&1 || {
    echo "FAIL: oxvba-jit miri"
    exit 1
}

echo "=== Running miri on oxvba-vm (semantics + broadword only) ==="
cargo +nightly miri test -p oxvba-vm -- semantics broadword 2>&1 || {
    echo "FAIL: oxvba-vm miri"
    exit 1
}

echo "=== All miri checks passed ==="
