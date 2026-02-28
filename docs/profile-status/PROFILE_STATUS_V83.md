# PROFILE_STATUS_V83.md

## Profile
- ID: mvp-array-call-and-paramarray-v83
- Ladder step: v83

## Scope Summary
- Added parser/typecheck/emit support for `ParamArray` parameter signatures with trailing positional packing.
- Added call-path diagnostics for unsupported named-argument ParamArray calls in this stage.
- Added runtime/conformance evidence for packed-count behavior via `UBound`.

## Gate Artifacts
- scripts/run-formal.ps1
- scripts/run-matrix.ps1
- docs/worksets/WORKSET_2026-02-28_ARRAY_CALL_PARAMARRAY_V83.md
- docs/evidence/profiles/v83/matrix_latest.csv
- docs/evidence/profiles/v83/gate_report.md
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile `v83` is complete when FO-V83-* obligations are pass, required VM/JIT matrix cells are green for profile scope, and strict async Kani run `v83-kani` is started and tracked as `DG-V83-001`.

## Gate Result (2026-02-28)
- `FO-V83-001..003`: pass (`docs/evidence/formal/latest_run.md`).
- Matrix gate: pass (`docs/evidence/profiles/v83/gate_report.md`, required cells `2/2` green).
- Deferred strict formal lane: started (`v83-kani`), register entry `DG-V83-001` (`docs/evidence/formal/DEFERRED_GATES.md`).
