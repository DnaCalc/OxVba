# WORKSET_2026-02-27_PERF_SHAPE_V26.md

## Profile
- ID: mvp-perf-shape-v26
- Ladder step: v26

## Purpose
Execute and stabilize profile scope: Perf-shape stabilization + v26 closure.

## Verification Commands
./scripts/run-conformance.ps1 -Backend vm
./scripts/run-conformance.ps1 -Backend jit
./scripts/run-matrix.ps1 -ProfileScope mvp-perf-shape-v26 -OutputDir docs/evidence/profiles/v26
./scripts/run-formal.ps1 -ProfileScope mvp-perf-shape-v26
./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal
