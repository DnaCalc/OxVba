# PROFILE_STATUS_V5.md

## Profile
- ID: `mvp-else-paths-v5`
- Ladder step: `v5`

## Scope Summary
- `Else` and `ElseIf` chains in `If` control flow.
- Emitter support for explicit else-block jump stitching.
- Conformance coverage for direct `Else`, `ElseIf`, and `ElseIf+Else` paths.

## Gate Artifacts
- Matrix report: `docs/evidence/profiles/v5/gate_report.md`
- Matrix CSV: `docs/evidence/profiles/v5/matrix_latest.csv`
- Formal run report: `docs/evidence/formal/latest_run.md`
- Formal run csv: `docs/evidence/formal/latest_run.csv`

## Closure Signals
A `v5` profile run is considered complete when:
1. `Else`/`ElseIf` fixtures are green for required backend cells.
2. Matrix gate report status is `PASS`.
3. Formal obligations `FO-V5-001..003` are executed and recorded (non-blocking policy active).
