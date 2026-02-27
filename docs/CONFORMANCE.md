# CONFORMANCE.md

## Purpose
Defines the current conformance loop and matrix gate for the OxVBA MVP profile.

## Assets
- `conformance/tests/*.bas` — executable input corpus.
- `conformance/golden/*.csv` — expected outcomes.
- `conformance/divergences/*.bas` — known mismatch fixtures tracked as evidence records.

Current corpus includes:
- MVP arithmetic smoke path.
- `Option Explicit` success case.
- `Option Explicit` undeclared-variable failure case.
- Integer subtraction path.

## Commands
```powershell
./scripts/run-smoke.ps1
./scripts/run-conformance.ps1
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1
```

## Current policy
At MVP stage, conformance compares:
- execution status (`ok` / `error`)
- slot snapshot output (`SLOTS:` line from CLI)

As runtime semantics mature, this will expand to richer structured outputs (error state and object lifecycle signals).

## Declared Profile Scope (Phase 12 gate)
- Profile id: `mvp-int32-core-v1`
- Platform: Windows x64
- Backends: `vm`, `jit` (JIT toggle path with VM-equivalent semantics)
- Required matrix cells:
  - `windows/x64/vm`
  - `windows/x64/jit`

Phase 12 gate is evaluated by `./scripts/run-matrix.ps1`, which writes:
- `docs/evidence/phase12/matrix_latest.csv`
- `docs/evidence/phase12/gate_report.md`
