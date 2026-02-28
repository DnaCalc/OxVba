# WORKSET_2026-02-28_ARRAY_TYPE_MODEL_V80.md

## Purpose
Execute profile `v80` (`mvp-array-type-model-v80`) in the `v67..v86` typing ladder.

## Scope
- Introduce a unified internal array descriptor model for typed and variant arrays.
- Record descriptor metadata (`element_type`, `rank`, `bounds`, `dynamic`) on bound procedures/modules.
- Keep current execution behavior stable while upgrading metadata used by later array semantics profiles.

## Implementation Targets
- `crates/oxvba-compiler/src/resolve.rs`
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/emit.rs`
- `crates/oxvba-compiler/src/optimize.rs`
- `docs/profile-status/PROFILE_STATUS_V80.md`

## Validation Commands
```powershell
cargo test
./scripts/run-formal.ps1 -ProfileScope mvp-array-type-model-v80
./scripts/run-matrix.ps1 -ProfileScope mvp-array-type-model-v80 -OutputDir docs/evidence/profiles/v80
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v80` closes when FO-V80-* obligations are pass, matrix gate cells are green for profile scope, and strict async Kani run `v80-kani` is started and tracked as deferred gate `DG-V80-001`.
