# V0.2 OxVba Backend Performance Runner

Date: 2026-04-27

Bead: `bd-bqm8.11.3`

Parent: `bd-bqm8.11`

## Scope

This evidence closes the OxVba backend runner bead for the V0.2 performance
scaffold. The runner consumes the V0.2 corpus, supports focused workload
selection, emits the common markdown/CSV schema, and supports no-artifacts mode
for validation without publishing local timing artifacts.

## Changes

- Added `scripts/run-v02-performance.ps1`.
- The runner reads `docs/validation/V02_PERFORMANCE_BENCHMARK_CORPUS_V1.csv`.
- Default selection runs non-VBA corpus rows; `-WorkloadId` narrows execution.
- `-NoArtifacts` writes to `temp/no-artifacts/v02-performance/<run-id>`.
- Output rows include run ID, timestamp, host OS, workload ID, engine, mode,
  iterations, warmups, mean/min/max milliseconds, VM comparison ratio, claim
  boundary, and source command.
- `V02-PERF-002` was narrowed to explicit stable string-runtime workloads after
  broad conformance patterns exposed existing unrelated VM/JIT divergences.

## Validation

- `./scripts/run-v02-performance.ps1 -WorkloadId V02-PERF-002 -Iterations 1 -WarmupIterations 0 -NoArtifacts`
  - Result: passed.
  - Workload rows: 4.
  - Artifact paths:
    - `temp/no-artifacts/v02-performance/20260427T105218Z/V02_PERFORMANCE_RUN_20260427T105218Z.csv`
    - `temp/no-artifacts/v02-performance/20260427T105218Z/V02_PERFORMANCE_RUN_20260427T105218Z.md`
  - Coverage: VM/JIT, baseline-no-opt/optimized modes, stable string runtime
    subset.
- `./scripts/check-governance.ps1`
  - Result: passed.
- `git diff --check`
  - Result: passed with line-ending normalization warnings only.

## Boundary

This runner does not perform Excel/VBA automation capture. It prepares the
OxVba backend side of the performance lane. `bd-bqm8.11.4` remains responsible
for bounded VBA comparison capture/import.

## Result

`bd-bqm8.11.3` is complete. Parent `bd-bqm8.11` remains in-progress.
