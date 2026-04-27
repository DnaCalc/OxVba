# V0.2 VBA Comparison Capture and Import Harness

Date: 2026-04-27

Bead: `bd-bqm8.11.4`

Parent: `bd-bqm8.11`

## Scope

This evidence closes the bounded VBA comparison capture/import bead. The lane
must support Excel/VBA timing rows when the host permits automation, but it must
also produce explicit skipped rows when Excel/VBA automation is unavailable so
portable validation remains deterministic.

## Changes

- Added `scripts/run-v02-vba-comparison.ps1`.
- The script reads VBA-comparison rows from
  `docs/validation/V02_PERFORMANCE_BENCHMARK_CORPUS_V1.csv`.
- `-SkipCapture` emits structured skipped rows for all VBA comparison workloads.
- `-ImportCsv` validates and normalizes externally captured VBA timing CSV rows.
- Direct Excel/VBA capture is implemented behind the default path using
  `Excel.Application` and generated VBA modules when the host supports COM
  automation and trusted VBProject access.
- `-NoArtifacts` writes outputs under
  `temp/no-artifacts/v02-vba-comparison/<run-id>`.

## Validation

- `./scripts/run-v02-vba-comparison.ps1 -SkipCapture -NoArtifacts`
  - Result: passed.
  - Rows: 3 skipped rows for `V02-PERF-005` through `V02-PERF-007`.
  - Artifacts:
    - `temp/no-artifacts/v02-vba-comparison/20260427T110255Z/V02_VBA_COMPARISON_RUN_20260427T110255Z.csv`
    - `temp/no-artifacts/v02-vba-comparison/20260427T110255Z/V02_VBA_COMPARISON_RUN_20260427T110255Z.md`
- `./scripts/run-v02-vba-comparison.ps1 -ImportCsv temp/no-artifacts/v02-vba-import-sample/sample.csv -NoArtifacts`
  - Result: passed.
  - Rows: 1 imported `V02-PERF-005` row normalized to the common schema.
- `./scripts/check-governance.ps1`
  - Result: passed.
- `git diff --check`
  - Result: passed with line-ending normalization warnings only.

## Boundary

This bead does not claim that Excel/VBA capture ran on the current machine.
The validated claim is that the harness has deterministic skip behavior,
structured import behavior, and a bounded direct capture implementation for
Windows hosts where Excel automation and VBProject access are available.

## Result

`bd-bqm8.11.4` is complete. Parent `bd-bqm8.11` remains in-progress.
