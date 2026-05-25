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

## Disabled JIT skeleton check
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
The repository is in active implementation. The interpreter is the current executable truth, and the old JIT implementation has been replaced by a disabled API skeleton pending a JIT v2 design.
