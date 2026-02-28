# PROFILE_STATUS_V84.md

## Profile
- ID: mvp-array-boundary-and-dispatch-v84
- Ladder step: v84

## Scope Summary
- Added deterministic array marshalling projection for dispatch invocation boundary when array-tag arguments are passed.
- Preserved scalar dispatch behavior while introducing SAFEARRAY-shape roundtrip helpers in runtime boundary utilities.
- Recorded v84 deferred-gate reconciliation poll state for array track DG runs (`v80..v83`).

## Gate Artifacts
- scripts/run-formal.ps1
- scripts/run-matrix.ps1
- docs/worksets/WORKSET_2026-02-28_ARRAY_BOUNDARY_DISPATCH_V84.md
- docs/evidence/profiles/v84/matrix_latest.csv
- docs/evidence/profiles/v84/gate_report.md
- docs/evidence/formal/latest_run.md
- docs/evidence/formal/EXTENDED_TODO.md

## Closure Signals
Profile `v84` is complete when FO-V84-* obligations are pass, required VM/JIT matrix cells are green for profile scope, and deferred-gate reconciliation status for `v80..v83` is documented.

## Gate Result (2026-02-28)
- `FO-V84-001..003`: pass (`docs/evidence/formal/latest_run.md`).
- Matrix gate: pass (`docs/evidence/profiles/v84/gate_report.md`, required cells `2/2` green).
- Deferred-gate reconciliation checkpoint (`v80..v83`): all four strict runs remain `dg-running`; follow-up tracked as `FTODO-V84-001` (`docs/evidence/formal/EXTENDED_TODO.md`).
