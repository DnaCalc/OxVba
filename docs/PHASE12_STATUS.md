# PHASE12_STATUS.md

## Phase
Phase 12: Conformance and Stabilization

## Declared profile scope
- Profile: `mvp-stabilization-rollup-v66`
- Platform class: `windows`
- Architecture: `x64`
- Required backends: `vm`, `jit`

## Gate criteria mapping
- Required matrix cells for declared profile scope are green.
  - Evidence artifact: `docs/evidence/profiles/v66/matrix_latest.csv`
  - Gate report: `docs/evidence/profiles/v66/gate_report.md`
- Integrated correctness/performance gate is green.
  - Integrated report: `docs/evidence/profiles/v66/integrated_gate.md`
  - Benchmark artifacts: `docs/evidence/profiles/v66/benchmark_latest.md`, `docs/evidence/profiles/v66/benchmark_latest.csv`
- Formal obligations up to scope are recorded in non-blocking mode.
  - Report: `docs/evidence/formal/latest_run.md`
  - CSV: `docs/evidence/formal/latest_run.csv`
- Remaining divergences are explicitly documented with evidence records.
  - Evidence index: `docs/evidence/divergences/README.md`
  - Records: `docs/evidence/divergences/DIV-0001.md`, `docs/evidence/divergences/DIV-0002.md`

## Status
This file is validated together with `./scripts/run-profile-gate.ps1` output and should be considered green only when the latest integrated report indicates `Final gate status: PASS`.

Latest recorded integrated gate (`docs/evidence/profiles/v66/integrated_gate.md`): `PASS`.
