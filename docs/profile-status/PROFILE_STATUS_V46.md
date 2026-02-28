# PROFILE_STATUS_V46.md

## Profile
- ID: mvp-stdlib-string-core-v46
- Ladder step: v46

## Scope Summary
- String-core intrinsic subset:
  - `Len`, `Left`, `Right`, `Mid`, `InStr`, `LCase`, `UCase`.
- Current runtime interprets values as decimal strings over integer slots for this profile subset.

## Gate Artifacts
- conformance/tests/stdlib_len_basic.bas
- conformance/tests/stdlib_slice_ops.bas
- conformance/tests/stdlib_instr_case_ops.bas
- docs/evidence/profiles/v46/matrix_latest.csv
- docs/evidence/profiles/v46/gate_report.md
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile v46 is complete when string-core subset conformance fixtures are green and `FO-V46-*` obligations are recorded in formal reports.
