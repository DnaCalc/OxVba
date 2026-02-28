# PROFILE_STATUS_V42.md

## Profile
- ID: mvp-lang-redim-preserve-v42
- Ladder step: v42

## Scope Summary
- Dynamic `ReDim` / `ReDim Preserve` one-dimensional literal-bound subset in current static-slot runtime model.

## Gate Artifacts
- conformance/tests/redim_preserve_keeps_values.bas
- conformance/tests/redim_without_preserve_resets.bas
- conformance/tests/redim_expand_allows_new_index.bas
- conformance/tests/redim_shrink_bounds_error.bas
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile v42 is complete when ReDim conformance fixtures are green and `FO-V42-*` obligations are recorded in formal reports.
