# WORKSET_2026-02-27_PARAMS_V9.md

## Profile
- ID: `mvp-params-v9`
- Ladder step: `v9`
- Formal level target: `F2`

## Purpose
Add a first executable parameter-passing model for procedure calls with clear `ByVal`/`ByRef` semantics.

## Scope
1. Parse procedure signatures with `ByVal`/`ByRef` parameter modifiers.
2. Parse call argument lists (`Call Foo(x, 1)`).
3. Typecheck arity and `ByRef` argument validity.
4. Emit argument binding logic at call sites with `ByRef` copy-back semantics.
5. Expand conformance/formal checks for pass-by-mode behavior.

## Exit Gate
1. ByVal/ByRef fixtures green in required backend cells.
2. Invalid ByRef argument cases fail deterministically.
3. Matrix report for v9 is `PASS`.
4. Formal obligations `FO-V9-001..003` recorded.

## Verification Commands
```powershell
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-params-v9 -OutputDir docs/evidence/profiles/v9
./scripts/run-formal.ps1 -ProfileScope mvp-params-v9
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
```
