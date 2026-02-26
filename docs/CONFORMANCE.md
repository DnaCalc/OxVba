# CONFORMANCE.md

## Purpose
Defines the current conformance loop for OxVBA MVP execution.

## Assets
- `conformance/tests/*.bas` — executable input corpus.
- `conformance/golden/*.csv` — expected outcomes.

Current corpus includes:
- MVP arithmetic smoke path.
- `Option Explicit` success case.
- `Option Explicit` undeclared-variable failure case.

## Commands
```powershell
./scripts/run-smoke.ps1
./scripts/run-conformance.ps1
```

## Current policy
At MVP stage, conformance compares:
- execution status (`ok` / `error`)
- slot snapshot output (`SLOTS:` line from CLI)

As runtime semantics mature, this will expand to richer structured outputs (error state and object lifecycle signals).
