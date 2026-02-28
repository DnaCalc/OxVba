# WORKSET_2026-02-28_ARRAY_BOUNDS_INDEXING_V81.md

## Purpose
Execute profile `v81` (`mvp-array-bounds-and-indexing-v81`) in the `v67..v86` typing ladder.

## Scope
- Add non-zero lower-bound support for array declarations and references (`Option Base`, explicit `lower To upper` bounds).
- Add multi-dimensional declaration/index parsing with deterministic linearization for current executable subset.
- Keep descriptor metadata coherent with runtime alias mapping for follow-on `ReDim` and parameter-array profiles.

## Implementation Targets
- `crates/oxvba-compiler/src/resolve.rs`
- `crates/oxvba-compiler/src/lib.rs`
- `conformance/tests/*.bas`
- `conformance/golden/smoke.csv`
- `docs/profile-status/PROFILE_STATUS_V81.md`

## Validation Commands
```powershell
cargo test
./scripts/run-formal.ps1 -ProfileScope mvp-array-bounds-and-indexing-v81
./scripts/run-matrix.ps1 -ProfileScope mvp-array-bounds-and-indexing-v81 -OutputDir docs/evidence/profiles/v81
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v81` closes when FO-V81-* obligations are pass, matrix gate cells are green for profile scope, and strict async Kani run `v81-kani` is started and tracked as deferred gate `DG-V81-001`.
