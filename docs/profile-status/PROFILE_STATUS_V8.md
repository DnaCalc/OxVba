# PROFILE_STATUS_V8.md

## Profile
- ID: `mvp-procedures-v8`
- Ladder step: `v8`

## Scope Summary
- Named `Sub`/`Function` body extraction.
- `Call <name>` dispatch to compiled procedure blocks.
- VM call stack handling via `CallProc`/`Return` opcodes.
- Per-procedure declaration slots for local-scope isolation.

## Gate Artifacts
- Matrix report: `docs/evidence/profiles/v8/gate_report.md`
- Matrix CSV: `docs/evidence/profiles/v8/matrix_latest.csv`
- Formal run report: `docs/evidence/formal/latest_run.md`
- Formal run csv: `docs/evidence/formal/latest_run.csv`

## Closure Signals
A `v8` profile run is considered complete when:
1. Procedure-call fixtures are green for required backend cells.
2. Matrix gate report status is `PASS`.
3. Formal obligations `FO-V8-001..003` are executed and recorded.
