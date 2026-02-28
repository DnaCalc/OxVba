# WORKSET_2026-02-28_PERF_HOTPATH_BASELINES_V64

## Profile
- ID: `mvp-perf-hotpath-baselines-v64`
- Ladder step: `v64`

## Scope
- Capture mixed workload baseline metrics for VM and JIT conformance runs.
- Emit benchmark CSV + markdown artifacts under profile evidence directory.

## Implementation Tasks
- Upgrade benchmark runner to record per-workload baseline/optimized timings.
- Add aggregate gain summary and profile scope metadata.
- Add formal checks for benchmark artifact integrity.

## Gate Commands
- `./scripts/run-bench.ps1 -ProfileScope mvp-perf-hotpath-baselines-v64 -OutputPath docs/evidence/profiles/v64/benchmark_latest.md -OutputCsvPath docs/evidence/profiles/v64/benchmark_latest.csv`
- `./scripts/run-formal.ps1 -ProfileScope mvp-perf-hotpath-baselines-v64`
- `./scripts/run-matrix.ps1 -ProfileScope mvp-perf-hotpath-baselines-v64 -OutputDir docs/evidence/profiles/v64`
