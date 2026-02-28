# WORKSET_2026-02-28_ARRAY_CALL_PARAMARRAY_V83.md

## Purpose
Execute profile `v83` (`mvp-array-call-and-paramarray-v83`) in the `v67..v86` typing ladder.

## Scope
- Add call-path support for `ParamArray` signature parsing and trailing-argument packing in current executable subset.
- Preserve existing named/optional/byref call behavior for non-ParamArray procedures.
- Add deterministic diagnostics for unsupported `ParamArray` named-argument flows in this stage.

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
./scripts/run-formal.ps1 -ProfileScope mvp-array-call-and-paramarray-v83
./scripts/run-matrix.ps1 -ProfileScope mvp-array-call-and-paramarray-v83 -OutputDir docs/evidence/profiles/v83
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v83` closes when FO-V83-* obligations are pass, matrix gate cells are green for profile scope, and strict async Kani run `v83-kani` is started and tracked as deferred gate `DG-V83-001`.
