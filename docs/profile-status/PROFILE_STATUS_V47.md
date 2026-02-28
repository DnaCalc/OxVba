# PROFILE_STATUS_V47.md

## Profile
- ID: mvp-stdlib-string-advanced-v47
- Ladder step: v47

## Scope Summary
- String-advanced intrinsic subset:
  - `Split`, `Join`, `Replace`, `Trim`, `LTrim`, `RTrim`, `StrComp`.
- Current runtime semantics use decimal-string-over-int projection for deterministic execution.

## Gate Artifacts
- conformance/tests/stdlib_advanced_split_join.bas
- conformance/tests/stdlib_advanced_replace_trim.bas
- conformance/tests/stdlib_advanced_strcomp.bas
- docs/evidence/profiles/v47/matrix_latest.csv
- docs/evidence/profiles/v47/gate_report.md
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile v47 is complete when advanced string subset conformance fixtures are green and `FO-V47-*` obligations are recorded in formal reports.
