#!/bin/bash
# Run cargo miri on unsafe-heavy crates.
# Excludes Windows-specific COM FFI paths (Windows APIs are incompatible with miri).
# The oxvba-com miri_variant_mock module provides pure-Rust mock paths for
# VARIANT layout verification without calling actual COM APIs.
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

echo "=== Running miri on oxvba-com (miri_variant mock paths) ==="
cargo +nightly miri test -p oxvba-com -- miri_variant 2>&1 || {
    echo "FAIL: oxvba-com miri"
    exit 1
}

echo "=== All miri checks passed ==="
