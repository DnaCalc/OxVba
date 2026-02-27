# WORKSET_2026-02-27_FULL_COVERAGE_PERF_GATE_V36.md

## Profile
- ID: mvp-full-coverage-perf-gate-v36
- Ladder step: v36

## Purpose
Execute and stabilize profile scope: Coverage+performance consolidation gate.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-full-coverage-perf-gate-v36 -OutputDir docs/evidence/profiles/v36
./scripts/run-formal.ps1 -ProfileScope mvp-full-coverage-perf-gate-v36
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
