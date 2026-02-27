# WORKSET_2026-02-27_ELSE_PATHS_V5.md

## Profile
- ID: `mvp-else-paths-v5`
- Ladder step: `v5`
- Formal level target: `F2`

## Purpose
Complete branch-chain semantics for `If` blocks by adding `Else` and `ElseIf` support with deterministic single-path execution.

## Scope
1. Parse and bind `Else` and `ElseIf` in structured condition trees.
2. Emit branch bytecode with correct jump targets for then/else chains.
3. Add conformance fixtures for `Else`, `ElseIf`, and `ElseIf + Else` paths.
4. Add executable formal model checks for branch determinism and model equivalence.

## Exit Gate
1. Compiler resolves and emits `Else`/`ElseIf` paths without unsupported fallbacks.
2. VM executes emitted control flow with expected single-path effects.
3. Conformance suite green in required backend cells.
4. Matrix report for v5 is `PASS`.
5. Formal obligations `FO-V5-001..003` recorded by formal lane.

## Verification Commands
```powershell
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-else-paths-v5 -OutputDir docs/evidence/profiles/v5
./scripts/run-formal.ps1 -ProfileScope mvp-else-paths-v5
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
```
