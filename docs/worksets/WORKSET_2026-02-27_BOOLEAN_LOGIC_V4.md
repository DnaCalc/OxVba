# WORKSET_2026-02-27_BOOLEAN_LOGIC_V4.md

## Profile
- ID: `mvp-boolean-logic-v4`
- Ladder step: `v4`
- Formal level target: `F1`

## Purpose
Expand condition semantics from equality-only branches to relational and boolean-composed conditions while preserving control-flow correctness.

## Scope
1. Relational operators in `If` conditions: `=`, `<>`, `<`, `<=`, `>`, `>=`.
2. Boolean composition in `If` conditions: `Not`, `And`, `Or`.
3. VM opcode support for comparison and boolean operations.
4. Conformance corpus expansion for relational/boolean condition cases.

## Exit Gate
1. Compiler emits comparison/boolean opcodes for new condition forms.
2. VM executes new opcodes correctly with unit coverage.
3. Conformance corpus includes relational/boolean fixtures and is green in required backend cells.
4. Matrix report for v4 is `PASS`.
5. Formal obligation `FO-V4-001` is executed and recorded under non-blocking policy.

## Verification Commands
```powershell
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-boolean-logic-v4 -OutputDir docs/evidence/profiles/v4
./scripts/run-formal.ps1 -ProfileScope mvp-boolean-logic-v4
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
```