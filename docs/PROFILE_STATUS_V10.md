# PROFILE_STATUS_V10.md

## Profile
- ID: `mvp-arrays-v10`
- Ladder step: `v10`

## Scope Summary
- `Dim a(n)` fixed-size integer arrays.
- Indexed element load/store syntax (`a(i)`), compile subset with integer indices.
- Bounds errors for out-of-range accesses.

## Gate Artifacts
- Matrix report: `docs/evidence/profiles/v10/gate_report.md`
- Matrix CSV: `docs/evidence/profiles/v10/matrix_latest.csv`
- Formal run report: `docs/evidence/formal/latest_run.md`
- Formal run csv: `docs/evidence/formal/latest_run.csv`

## Closure Signals
A `v10` profile run is complete when:
1. Array fixtures are green for required backend cells.
2. Matrix gate report status is `PASS`.
3. Formal obligations `FO-V10-001..003` are executed and recorded.
