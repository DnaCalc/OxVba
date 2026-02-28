# PROFILE_STATUS_V64.md

## Profile
- ID: mvp-perf-hotpath-baselines-v64
- Ladder step: v64

## Scope Summary
- Capture mixed VM/JIT conformance workload timing baselines with per-workload benchmark artifacts.

## Gate Artifacts
- scripts/run-bench.ps1
- docs/evidence/profiles/v64/benchmark_latest.md
- docs/evidence/profiles/v64/benchmark_latest.csv
- docs/evidence/profiles/v64/matrix_latest.csv
- docs/evidence/profiles/v64/gate_report.md
- docs/evidence/formal/latest_run.md

## Closure Signals
Profile v64 is complete when FO-V64-* obligations are pass and required matrix cells for `v64` are green.
