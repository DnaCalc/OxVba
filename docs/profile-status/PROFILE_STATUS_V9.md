# PROFILE_STATUS_V9.md

## Profile
- ID: `mvp-params-v9`
- Ladder step: `v9`

## Scope Summary
- Procedure parameter parsing (`ByVal`, `ByRef`, default `ByRef`).
- Call argument binding and arity checks.
- `ByRef` variable-argument validation.
- Call-site mutation propagation semantics for `ByRef`.

## Gate Artifacts
- Matrix report: `docs/evidence/profiles/v9/gate_report.md`
- Matrix CSV: `docs/evidence/profiles/v9/matrix_latest.csv`
- Formal run report: `docs/evidence/formal/latest_run.md`
- Formal run csv: `docs/evidence/formal/latest_run.csv`

## Closure Signals
A `v9` profile run is complete when:
1. Parameter fixtures are green for required backend cells.
2. Matrix gate report is `PASS`.
3. Formal obligations `FO-V9-001..003` are executed and recorded.
