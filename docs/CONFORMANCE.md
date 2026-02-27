# CONFORMANCE.md

## Purpose
Defines the current conformance loop and matrix gate for the active ladder profile.

## Assets
- `conformance/tests/*.bas` — executable input corpus.
- `conformance/golden/*.csv` — expected outcomes.
- `conformance/divergences/*.bas` — divergence/regression fixtures tracked in evidence docs.

Current corpus includes:
- MVP arithmetic smoke path.
- `Option Explicit` success case.
- `Option Explicit` undeclared-variable failure case.
- Integer subtraction path.
- `If ... Then ... End If` branch behavior.
- `For ... Next` loop behavior (including zero-iteration case).
- Nested `If` inside `For`.
- Relational operator branches (`<>`, `<`, `>=`).
- Boolean condition composition (`Not`, `And`, `Or`).
- `Else` and `ElseIf` branch-chain selection.
- `Do While ... Loop`, `Do ... Loop While`, and `Exit Do`.
- `Select Case` constant dispatch with `Case Else`.

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

## Declared Profile Scope (Current Gate)
- Profile id: `mvp-select-case-v7`
- Platform: Windows x64
- Backends: `vm`, `jit` (JIT toggle path with VM-equivalent semantics)
- Required matrix cells:
  - `windows/x64/vm`
  - `windows/x64/jit`

Current profile gate is evaluated by `./scripts/run-matrix.ps1`, which writes:
- `docs/evidence/profiles/v7/matrix_latest.csv`
- `docs/evidence/profiles/v7/gate_report.md`
