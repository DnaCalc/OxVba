# PROFILE_STATUS_V70.md

## Profile
- ID: mvp-typing-procedure-signatures-v70
- Ladder step: v70

## Scope Summary
- Add procedure return-type metadata for typed function signatures.
- Apply function return precedence (`As` > type char > `Def*` > `Variant`) and bind typed function return assignment symbol.
- Tighten `ByRef` legality under typed arguments: variable-only and exact type match for non-Variant `ByRef` parameters.

## Gate Artifacts
- scripts/run-formal.ps1
- scripts/run-matrix.ps1
- docs/worksets/WORKSET_2026-02-28_PROCEDURE_SIGNATURES_V70.md
- docs/evidence/profiles/v70/matrix_latest.csv
- docs/evidence/profiles/v70/gate_report.md
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile `v70` is complete when FO-V70-* obligations are pass, required VM/JIT matrix cells are green for profile scope, and typed procedure signature fixtures are green.
