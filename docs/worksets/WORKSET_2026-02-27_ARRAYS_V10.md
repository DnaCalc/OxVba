# WORKSET_2026-02-27_ARRAYS_V10.md

## Profile
- ID: `mvp-arrays-v10`
- Ladder step: `v10`
- Formal level target: `F1`

## Purpose
Add first fixed-size array semantics to the executable subset with deterministic bounds behavior.

## Scope
1. Parse `Dim name(maxIndex)` declarations for integer arrays.
2. Parse indexed references in assignment and expression positions.
3. Map array elements to stable storage slots.
4. Reject out-of-range accesses deterministically.
5. Expand conformance/formal coverage for array behavior.

## Exit Gate
1. Array store/load fixtures green.
2. Bounds-violation fixture errors deterministically.
3. Matrix report for v10 is `PASS`.
4. Formal obligations `FO-V10-001..003` recorded.

## Verification Commands
```powershell
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-arrays-v10 -OutputDir docs/evidence/profiles/v10
./scripts/run-formal.ps1 -ProfileScope mvp-arrays-v10
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
```
