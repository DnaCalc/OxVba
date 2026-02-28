# PROFILE_STATUS_V75.md

## Profile
- ID: mvp-typing-call-coercion-early-late-v75
- Ladder step: v75

## Scope Summary
- Unify argument coercion checks across early and mixed typed procedure calls.
- Make late-call argument-pack coercion explicit (while retaining non-executable late-call runtime policy).
- Add table-backed verification for call coercion mode rules.

## Gate Artifacts
- scripts/run-formal.ps1
- scripts/run-matrix.ps1
- docs/worksets/WORKSET_2026-02-28_CALL_COERCION_EARLY_LATE_V75.md
- docs/evidence/profiles/v75/matrix_latest.csv
- docs/evidence/profiles/v75/gate_report.md
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile `v75` is complete when FO-V75-* obligations are pass, required VM/JIT matrix cells are green for profile scope, and call coercion table-alignment tests are green.
