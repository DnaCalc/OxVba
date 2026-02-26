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

## One-command project check
```powershell
./scripts/meta-check.ps1
```

## Current state
The repository is in active bootstrap/implementation. Crate APIs are present and compile, while full runtime semantics are being implemented phase-by-phase per `MACH1000_PLAN.md`.
