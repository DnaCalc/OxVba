# WORKSET_2026-02-28_COERCION_MATRIX_V73.md

## Purpose
Execute profile `v73` (`mvp-typing-coercion-matrix-v73`) in the `v67..v86` typing ladder.

## Scope
- Introduce explicit coercion-matrix result classification in typecheck.
- Align decision-table rows (`tables/coercion.csv`) with assignment/argument coercion behavior.
- Expand assignment/argument coercion regression coverage for object/non-object mismatch paths.

## Implementation Targets
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/lib.rs`
- `tables/coercion.csv`
- `conformance/tests/coercion_assign_object_to_long_error.bas`
- `conformance/tests/coercion_arg_object_to_long_error.bas`
- `conformance/golden/smoke.csv`
- `docs/profile-status/PROFILE_STATUS_V73.md`

## Validation Commands
```powershell
cargo test -p oxvba-compiler
./scripts/run-formal.ps1 -ProfileScope mvp-typing-coercion-matrix-v73
./scripts/run-matrix.ps1 -ProfileScope mvp-typing-coercion-matrix-v73 -OutputDir docs/evidence/profiles/v73
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v73` closes when FO-V73-* obligations are pass, `v73` matrix cells are green, and coercion table rows are validated against runtime typecheck behavior.
