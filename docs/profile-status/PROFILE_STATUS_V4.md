# PROFILE_STATUS_V4.md

## Profile
- ID: `mvp-boolean-logic-v4`
- Ladder step: `v4`

## Scope Summary
- Relational operators in branch conditions.
- Boolean composition (`Not`, `And`, `Or`) in branch conditions.
- VM opcode support for comparison + boolean operations.

## Gate Artifacts
- Matrix report: `docs/evidence/profiles/v4/gate_report.md`
- Matrix CSV: `docs/evidence/profiles/v4/matrix_latest.csv`
- Formal run report: `docs/evidence/formal/latest_run.md`
- Formal run csv: `docs/evidence/formal/latest_run.csv`

## Closure Signals
A `v4` profile run is considered complete when:
1. Relational/boolean conformance fixtures are green for required backend cells.
2. Matrix gate report status is `PASS`.
3. Formal obligation `FO-V4-001` is executed and recorded (non-blocking policy active).