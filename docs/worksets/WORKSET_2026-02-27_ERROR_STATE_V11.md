# WORKSET_2026-02-27_ERROR_STATE_V11.md

## Profile
- ID: `mvp-error-state-v11`
- Ladder step: `v11`
- Formal level target: `F2`

## Purpose
Introduce explicit runtime error-state behavior for the MVP subset so control flow can continue under `Resume Next` and expose `Err.Number`.

## Scope
1. Parse/emit `On Error Resume Next`.
2. Parse/emit `Error <code>` runtime error instruction.
3. Add runtime error-state fields for mode and last error code.
4. Support reading `Err.Number` via expression path.
5. Add conformance and formal checks for default fail-vs-resume semantics.

## Exit Gate
1. Error-state fixtures green in required backend cells.
2. Default error mode fails on raised error.
3. Resume-next mode continues and updates `Err.Number`.
4. Matrix report for v11 is `PASS`.
5. Formal obligations `FO-V11-001..003` recorded.

## Verification Commands
```powershell
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-error-state-v11 -OutputDir docs/evidence/profiles/v11
./scripts/run-formal.ps1 -ProfileScope mvp-error-state-v11
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
```
