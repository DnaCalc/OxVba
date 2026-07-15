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

## JIT backend check
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
The repository is in active implementation. `oxvba-vm3` is the typed-OxIR
interpreter and the product reference runtime; `oxvba-jit` is a real Cranelift
backend that lowers the same linked `OxProgram` set to native code (no VM
fallback) and is checked for parity against vm3 by the `oxvba-differential`
harness. See [ARCHITECTURE.md](ARCHITECTURE.md) for the current stack.
