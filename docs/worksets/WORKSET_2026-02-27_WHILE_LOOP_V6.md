# WORKSET_2026-02-27_WHILE_LOOP_V6.md

## Profile
- ID: `mvp-while-loop-v6`
- Ladder step: `v6`
- Formal level target: `F2`

## Purpose
Add loop semantics beyond `For ... Next` through `Do While`, post-condition loops, and explicit `Exit Do` control.

## Scope
1. Resolver support for `Do While ... Loop` and `Do ... Loop While`.
2. Bound representation for loop form and `Exit Do` statement.
3. Emitter support for pre/post-condition loop control flow and loop-local exit patches.
4. Conformance coverage for loop behavior and short-circuit exit.
5. Formal obligations validating loop model behavior over reduced domains.

## Exit Gate
1. Compiler emits valid jump structures for pre/post loops.
2. `Exit Do` exits the innermost `Do` loop.
3. Conformance loop fixtures green in required backend cells.
4. Matrix report for v6 is `PASS`.
5. Formal obligations `FO-V6-001..003` recorded.

## Verification Commands
```powershell
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-while-loop-v6 -OutputDir docs/evidence/profiles/v6
./scripts/run-formal.ps1 -ProfileScope mvp-while-loop-v6
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
```
