# WORKSET_2026-02-28_OPERATOR_RESULT_RULES_V74.md

## Purpose
Execute profile `v74` (`mvp-typing-operator-result-rules-v74`) in the `v67..v86` typing ladder.

## Scope
- Enforce arithmetic operator typing checks for the current expression subset (`x +/- const`).
- Enforce comparison-operand compatibility checks in condition typing.
- Align operator result decisions with machine-checkable decision tables:
  - `tables/arithmetic.csv`
  - `tables/comparison.csv`
- Expand operator mismatch coverage in compiler and conformance fixtures.

## Implementation Targets
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/lib.rs`
- `tables/arithmetic.csv`
- `tables/comparison.csv`
- `conformance/tests/operator_arithmetic_object_plus_error.bas`
- `conformance/tests/operator_comparison_object_long_error.bas`
- `conformance/golden/smoke.csv`
- `docs/profile-status/PROFILE_STATUS_V74.md`

## Validation Commands
```powershell
cargo test -p oxvba-compiler
./scripts/run-formal.ps1 -ProfileScope mvp-typing-operator-result-rules-v74
./scripts/run-matrix.ps1 -ProfileScope mvp-typing-operator-result-rules-v74 -OutputDir docs/evidence/profiles/v74
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v74` closes when FO-V74-* obligations are pass, `v74` matrix cells are green, and arithmetic/comparison operator decision tables match typecheck behavior.
