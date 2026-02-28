# WORKSET_2026-02-28_CONVERSION_INTRINSICS_V76.md

## Purpose
Execute profile `v76` (`mvp-typing-conversion-intrinsics-v76`) in the `v67..v86` typing ladder.

## Scope
- Add typed result inference for conversion intrinsics:
  - `CInt`, `CLng`, `CDbl`, `CStr`, `CBool`, `CDate`, `Val`, `Str`, `CVErr`.
- Validate conversion intrinsic argument admissibility through the shared coercion engine.
- Add table-backed conversion intrinsic mapping checks.
- Perform deferred-gate reconciliation poll for `v73..v75`.

## Implementation Targets
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/lib.rs`
- `tables/conversion_intrinsics.csv`
- `tables/README.md`
- `conformance/tests/conversion_cint_to_object_error.bas`
- `conformance/golden/smoke.csv`
- `docs/evidence/formal/EXTENDED_TODO.md`
- `docs/profile-status/PROFILE_STATUS_V76.md`

## Validation Commands
```powershell
cargo test -p oxvba-compiler
./scripts/run-formal.ps1 -ProfileScope mvp-typing-conversion-intrinsics-v76
./scripts/run-matrix.ps1 -ProfileScope mvp-typing-conversion-intrinsics-v76 -OutputDir docs/evidence/profiles/v76
./scripts/meta-check.ps1 -Fast
```

## Closure Signals
`v76` closes when FO-V76-* obligations are pass, `v76` matrix cells are green, conversion intrinsic table rows align with typecheck behavior, and `v73..v75` deferred-gate reconciliation poll status is recorded.
