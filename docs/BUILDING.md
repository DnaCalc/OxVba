# BUILDING.md

## Prerequisites
- Rust toolchain (`rustup`, stable)
- Git
- PowerShell (for repo scripts on Windows)

## Build
```powershell
cargo check --workspace
cargo test --workspace
```

## JIT-specific testing
```powershell
cargo test -p oxvba-jit
```

## Release / benchmark builds
```powershell
cargo test --workspace --release
```

## Formal verification (WSL / Linux only)
Kani proofs live under `formal/`. Kani requires a Linux environment — use WSL on Windows:
```bash
cargo kani --workspace
```

## One-command project check
```powershell
./scripts/meta-check.ps1
```

## Current state
The repository is in active implementation. All crates compile, the interpreter covers 152 instructions, the JIT backend has full instruction parity (155 mapping entries), and the test suite has 2025+ tests with zero failures.
