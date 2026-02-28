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
- `GoSub`/`Return` intra-procedure flow subset.
- `Select Case` constant dispatch with `Case Else`.
- Named `Sub`/`Function` declarations and `Call` dispatch.
- `ByVal`/`ByRef` parameter passing subset.
- Trailing `Optional` parameter defaults (integer literal subset).
- Named argument call binding (`name := expr`) with ordering validation.
- `Property Get/Let/Set` declaration subset with assignment-form routing to `Let/Set`.
- Intrinsic conversion subset: `CInt`, `CLng`, `CDbl`, `CStr`, `CBool`, `CDate`, `Val`, `Str` (current int-domain semantics).
- String-core intrinsic subset: `Len`, `Left`, `Right`, `Mid`, `InStr`, `LCase`, `UCase` (decimal-string-over-int semantics).
- Fixed-size arrays with indexed load/store and bounds errors.
- Dynamic `ReDim` / `ReDim Preserve` (1D literal-bound subset).
- Module-level `Const` and `Enum` declaration usage subset.
- `Type ... End Type` declaration-block parse acceptance baseline.
- `On Error Resume Next` and `Err.Number` subset behavior.
- `On Error GoTo 0` reset behavior and `Resume Next` statement subset.
- `On Error GoTo <label>` handler transfer subset.

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
- Profile id: `mvp-full-coverage-perf-gate-v36`
- Platform: Windows x64
- Backends: `vm`, `jit` (JIT toggle path with VM-equivalent semantics)
- Required matrix cells:
  - `windows/x64/vm`
  - `windows/x64/jit`

Current profile gate is evaluated by `./scripts/run-matrix.ps1`, which writes:
- `docs/evidence/profiles/v36/matrix_latest.csv`
- `docs/evidence/profiles/v36/gate_report.md`
