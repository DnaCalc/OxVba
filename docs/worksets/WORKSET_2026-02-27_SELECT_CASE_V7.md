# WORKSET_2026-02-27_SELECT_CASE_V7.md

## Profile
- ID: `mvp-select-case-v7`
- Ladder step: `v7`
- Formal level target: `F2`

## Purpose
Add deterministic multi-branch dispatch via `Select Case` while preserving first-match behavior and explicit fallback handling.

## Scope
1. Bind `Select Case` with constant integer case values.
2. Support multi-value arms (`Case 1, 3`).
3. Support `Case Else`.
4. Emit executable dispatch branches with first-match semantics.
5. Expand conformance + formal obligations for determinism and fallback.

## Exit Gate
1. Select-case syntax compiles and executes on VM/JIT-toggle paths.
2. Conformance fixtures for basic/multi/else paths are green.
3. Matrix report for v7 is `PASS`.
4. Formal obligations `FO-V7-001..003` recorded.

## Verification Commands
```powershell
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-select-case-v7 -OutputDir docs/evidence/profiles/v7
./scripts/run-formal.ps1 -ProfileScope mvp-select-case-v7
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
```
