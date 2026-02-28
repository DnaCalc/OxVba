# WORKSET_2026-02-28_CONDITIONAL_COMPILATION_V61

## Profile
- ID: `mvp-lang-conditional-compilation-v61`
- Ladder step: `v61`

## Scope
- Add compile-time conditional directive handling in source normalization.
- Support `#Const`, `#If ... Then`, `#ElseIf ... Then`, `#Else`, and `#End If` subset semantics.
- Ensure inactive branches are excluded before semantic analysis.

## Implementation Tasks
- Extend resolver normalization pass with directive evaluation stack.
- Add deterministic expression evaluator for conditional compilation expressions.
- Add conformance fixture and compiler/host tests for branch selection behavior.

## Gate Commands
- `cargo test -p oxvba-compiler`
- `cargo test -p oxvba-host --lib`
- `./scripts/run-formal.ps1 -ProfileScope mvp-lang-conditional-compilation-v61`
- `./scripts/run-matrix.ps1 -ProfileScope mvp-lang-conditional-compilation-v61 -OutputDir docs/evidence/profiles/v61`
