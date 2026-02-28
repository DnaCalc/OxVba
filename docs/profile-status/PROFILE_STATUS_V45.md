# PROFILE_STATUS_V45.md

## Profile
- ID: mvp-stdlib-conversion-core-v45
- Ladder step: v45

## Scope Summary
- Intrinsic conversion function subset in current int-domain runtime:
  - `CInt`, `CLng`, `CDbl`, `CStr`, `CBool`, `CDate`, `Val`, `Str`.
- Conversion wrappers are parsed as expression-level intrinsics and lowered to existing integer expression model.

## Gate Artifacts
- conformance/tests/conversion_cint_basic.bas
- conformance/tests/conversion_nested_clng_cint.bas
- conformance/tests/conversion_val_str_subset.bas
- docs/evidence/profiles/v45/matrix_latest.csv
- docs/evidence/profiles/v45/gate_report.md
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile v45 is complete when conversion subset conformance fixtures are green and `FO-V45-*` obligations are recorded in formal reports.
