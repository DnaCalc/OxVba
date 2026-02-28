# WORKSET_2026-02-28_EARLY_LATE_CLASSIFICATION_V71.md

## Purpose
Execute profile `v71` (`mvp-typing-early-late-classification-v71`) in the `v67..v86` typing ladder.

## Scope
- Introduce deterministic call-site classification (`Early`, `Mixed`, `Late`) based on procedure signatures and in-scope declaration typing.
- Track mixed-call classification for signature/argument shapes involving dynamic (`Variant`/`Object`) flow.
- Detect late/default-member call targets on object-like symbols and emit explicit current-runtime diagnostics.

## Implementation Targets
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/lib.rs`
- `conformance/tests/late_bound_default_member_error.bas`
- `conformance/golden/smoke.csv`
- `docs/profile-status/PROFILE_STATUS_V71.md`

## Validation Commands
```powershell
cargo test -p oxvba-compiler
./scripts/run-formal.ps1 -ProfileScope mvp-typing-early-late-classification-v71
./scripts/run-matrix.ps1 -ProfileScope mvp-typing-early-late-classification-v71 -OutputDir docs/evidence/profiles/v71
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v71` closes when FO-V71-* obligations are pass, `v71` matrix cells are green, and early/mixed/late classification fixtures are stable and deterministic.
