# V0.2 Performance Thresholds and Trend Surfaces

Date: 2026-04-27

Bead: `bd-bqm8.11.5`

Parent: `bd-bqm8.11`

## Scope

This evidence closes the thresholds and trend-surface bead for the V0.2
performance scaffold. It publishes the threshold policy and adds an executable
summary script that classifies backend and VBA comparison artifacts.

## Changes

- Added `docs/validation/V02_PERFORMANCE_THRESHOLDS_V1.md`.
- Added `docs/validation/V02_PERFORMANCE_THRESHOLDS_V1.csv`.
- Added `scripts/summarize-v02-performance.ps1`.

## Validation

- `./scripts/run-v02-performance.ps1 -WorkloadId V02-PERF-002 -Iterations 1 -WarmupIterations 0 -NoArtifacts -RunId v02-threshold-check`
  - Result: passed.
  - Artifact:
    `temp/no-artifacts/v02-performance/v02-threshold-check/V02_PERFORMANCE_RUN_v02-threshold-check.csv`
- `./scripts/run-v02-vba-comparison.ps1 -SkipCapture -NoArtifacts -RunId v02-threshold-check`
  - Result: passed.
  - Artifact:
    `temp/no-artifacts/v02-vba-comparison/v02-threshold-check/V02_VBA_COMPARISON_RUN_v02-threshold-check.csv`
- `./scripts/summarize-v02-performance.ps1 -BackendCsv <backend-csv> -VbaCsv <vba-csv> -NoArtifacts -RunId v02-threshold-check`
  - Result: passed with overall `warn`.
  - Artifact:
    `temp/no-artifacts/v02-performance-summary/v02-threshold-check/V02_PERFORMANCE_SUMMARY_v02-threshold-check.md`
  - Classification:
    backend schema, backend rows, JIT/VM ratio, and product-claim policy passed;
    VBA comparison warned because only skipped rows were present on this host.
- `./scripts/check-governance.ps1`
  - Result: passed.
- `git diff --check`
  - Result: passed with line-ending normalization warnings only.

## Boundary

The trend summary can produce `warn` when only skipped VBA rows are present.
That is acceptable on hosts without Excel/VBA automation and does not support a
VBA speed claim. A `fail` means the artifact schema or row surface is too
incomplete for V0.2 performance reporting.

## Result

`bd-bqm8.11.5` is complete. Parent `bd-bqm8.11` remains in-progress.
