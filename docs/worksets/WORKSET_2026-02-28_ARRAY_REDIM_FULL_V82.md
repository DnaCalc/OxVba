# WORKSET_2026-02-28_ARRAY_REDIM_FULL_V82.md

## Purpose
Execute profile `v82` (`mvp-array-redim-full-v82`) in the `v67..v86` typing ladder.

## Scope
- Implement `ReDim Preserve` legality checks for current in-scope array model:
  - rank must stay stable,
  - only final-dimension upper bound may change,
  - lower bounds must remain stable.
- Preserve overlap semantics during size transitions and clear removed/new tail slots to avoid stale resurrection.
- Extend conformance and formal coverage for multidimensional preserve and shrink/expand behavior.

## Implementation Targets
- `crates/oxvba-compiler/src/resolve.rs`
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/emit.rs`
- `crates/oxvba-host/src/engine.rs`
- `conformance/tests/*.bas`
- `conformance/golden/smoke.csv`

## Validation Commands
```powershell
cargo test
./scripts/run-formal.ps1 -ProfileScope mvp-array-redim-full-v82
./scripts/run-matrix.ps1 -ProfileScope mvp-array-redim-full-v82 -OutputDir docs/evidence/profiles/v82
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v82` closes when FO-V82-* obligations are pass, matrix gate cells are green for profile scope, and strict async Kani run `v82-kani` is started and tracked as deferred gate `DG-V82-001`.
