# PROFILE_STATUS_V71.md

## Profile
- ID: mvp-typing-early-late-classification-v71
- Ladder step: v71

## Scope Summary
- Add call-mode classification (`Early`, `Mixed`, `Late`) for deterministic typed call routing.
- Keep current executable behavior for early/mixed paths.
- Emit explicit diagnostics for late/default-member call targets that are classified but not yet executable.

## Gate Artifacts
- scripts/run-formal.ps1
- scripts/run-matrix.ps1
- docs/worksets/WORKSET_2026-02-28_EARLY_LATE_CLASSIFICATION_V71.md
- docs/evidence/profiles/v71/matrix_latest.csv
- docs/evidence/profiles/v71/gate_report.md
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile `v71` is complete when FO-V71-* obligations are pass, required VM/JIT matrix cells are green for profile scope, and call-classification diagnostics remain deterministic.
