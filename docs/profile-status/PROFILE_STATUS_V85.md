# PROFILE_STATUS_V85.md

## Profile
- ID: mvp-typed-execution-fastpaths-v85
- Ladder step: v85

## Scope Summary
- Added VM typed hot-path helpers for frequent integer slot operations with fallback to canonical handlers.
- Added execution API to run VM with typed fast-paths explicitly enabled/disabled for parity validation.
- Added typed hot-loop conformance fixture and VM/JIT equivalence checks for specialized execution surface.

## Gate Artifacts
- scripts/run-formal.ps1
- scripts/run-matrix.ps1
- scripts/run-bench.ps1
- docs/worksets/WORKSET_2026-02-28_TYPED_EXEC_FASTPATHS_V85.md
- docs/evidence/profiles/v85/matrix_latest.csv
- docs/evidence/profiles/v85/gate_report.md
- docs/evidence/profiles/v85/benchmark_latest.md
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile `v85` is complete when FO-V85-* obligations are pass, required VM/JIT matrix cells are green for profile scope, benchmark artifacts are recorded for `v85`, and strict async Kani run `v85-kani` is started and tracked as `DG-V85-001`.

## Gate Result (2026-02-28)
- `FO-V85-001..003`: pass (`docs/evidence/formal/latest_run.md`).
- Matrix gate: pass (`docs/evidence/profiles/v85/gate_report.md`, required cells `2/2` green).
- Benchmark capture: pass (`docs/evidence/profiles/v85/benchmark_latest.md`, aggregate gain `0.31%`).
- Deferred strict formal lane: started (`v85-kani`), register entry `DG-V85-001` (`docs/evidence/formal/DEFERRED_GATES.md`).
