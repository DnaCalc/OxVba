# WORKSET_2026-02-28_ARRAY_BOUNDARY_DISPATCH_V84.md

## Purpose
Execute profile `v84` (`mvp-array-boundary-and-dispatch-v84`) in the `v67..v86` typing ladder.

## Scope
- Add deterministic array-boundary marshalling behavior for dispatch invocation path when array-tag payloads cross host boundary intrinsics.
- Preserve scalar dispatch behavior while introducing explicit array-tag marshalling projection for current SAFEARRAY subset model.
- Reconcile deferred-gate checkpoint for array track strict runs (`v80..v83`) and record live-state follow-up.

## Implementation Targets
- `crates/oxvba-runtime/src/safe_array.rs`
- `crates/oxvba-vm/src/interpreter.rs`
- `crates/oxvba-compiler/src/emit.rs`
- `crates/oxvba-host/src/engine.rs`
- `conformance/tests/*.bas`
- `conformance/golden/smoke.csv`

## Validation Commands
```powershell
cargo test
./scripts/run-formal.ps1 -ProfileScope mvp-array-boundary-and-dispatch-v84
./scripts/run-matrix.ps1 -ProfileScope mvp-array-boundary-and-dispatch-v84 -OutputDir docs/evidence/profiles/v84
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v84` closes when FO-V84-* obligations are pass, matrix gate cells are green for profile scope, and deferred-gate reconciliation status for `v80..v83` is recorded (`DG` register + `EXTENDED_TODO` if still running).
