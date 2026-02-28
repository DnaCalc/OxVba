# WORKSET_2026-02-28_CALL_COERCION_EARLY_LATE_V75.md

## Purpose
Execute profile `v75` (`mvp-typing-call-coercion-early-late-v75`) in the `v67..v86` typing ladder.

## Scope
- Make call-argument coercion explicit and mode-aware across:
  - early-bound calls,
  - mixed calls,
  - late-bound fallback argument packing.
- Keep call coercion behavior table-backed and deterministic.
- Extend coverage for mixed-call coercion and late-call named-argument classification.

## Implementation Targets
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/lib.rs`
- `tables/call_coercion.csv`
- `tables/README.md`
- `conformance/tests/call_coercion_mixed_variant_to_long.bas`
- `conformance/tests/late_call_named_argument_error.bas`
- `conformance/golden/smoke.csv`
- `docs/profile-status/PROFILE_STATUS_V75.md`

## Validation Commands
```powershell
cargo test -p oxvba-compiler
./scripts/run-formal.ps1 -ProfileScope mvp-typing-call-coercion-early-late-v75
./scripts/run-matrix.ps1 -ProfileScope mvp-typing-call-coercion-early-late-v75 -OutputDir docs/evidence/profiles/v75
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v75` closes when FO-V75-* obligations are pass, `v75` matrix cells are green, and `tables/call_coercion.csv` rows match mode-aware call coercion behavior in typecheck.
