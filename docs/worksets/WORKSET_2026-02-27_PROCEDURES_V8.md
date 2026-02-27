# WORKSET_2026-02-27_PROCEDURES_V8.md

## Profile
- ID: `mvp-procedures-v8`
- Ladder step: `v8`
- Formal level target: `F1`

## Purpose
Add executable multi-procedure structure: named procedure bodies, call dispatch, return-to-caller flow, and local declaration isolation.

## Scope
1. Resolve named `Sub`/`Function` bodies into procedure inventory.
2. Add bound `Call` statements and validation of call targets.
3. Emit `CallProc` and `Return` opcodes for non-entry procedures.
4. Extend VM interpreter with call stack handling.
5. Add conformance and formal checks for call/return and local-scope isolation.

## Exit Gate
1. Procedure source with call chains compiles and executes.
2. VM and JIT-toggle conformance cells are green for procedure fixtures.
3. Matrix report for v8 is `PASS`.
4. Formal obligations `FO-V8-001..003` are recorded.

## Verification Commands
```powershell
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-procedures-v8 -OutputDir docs/evidence/profiles/v8
./scripts/run-formal.ps1 -ProfileScope mvp-procedures-v8
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
```
