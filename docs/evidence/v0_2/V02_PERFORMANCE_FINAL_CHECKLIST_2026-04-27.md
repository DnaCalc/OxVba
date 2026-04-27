# V0.2 Performance and VBA Comparison Final Checklist

Date: 2026-04-27

Bead: `bd-bqm8.11.6`

Parent: `bd-bqm8.11`

## Scope

This checklist closes the V0.2 performance scaffold and VBA comparison harness
lane. It verifies that the corpus, OxVba backend runner, bounded VBA
capture/import harness, threshold policy, and summary surface are all present
and executable with explicit product-claim boundaries.

## Validation

- `./scripts/run-v02-performance.ps1 -WorkloadId V02-PERF-002 -Iterations 1 -WarmupIterations 0 -NoArtifacts -RunId v02-final-check`
  - Result: passed.
  - Artifact:
    `temp/no-artifacts/v02-performance/v02-final-check/V02_PERFORMANCE_RUN_v02-final-check.csv`
  - Coverage: stable string-runtime subset across VM/JIT and baseline/optimized
    modes.
- `./scripts/run-v02-vba-comparison.ps1 -SkipCapture -NoArtifacts -RunId v02-final-check`
  - Result: passed.
  - Artifact:
    `temp/no-artifacts/v02-vba-comparison/v02-final-check/V02_VBA_COMPARISON_RUN_v02-final-check.csv`
  - Coverage: deterministic skipped rows for all VBA comparison workloads.
- `./scripts/summarize-v02-performance.ps1 -BackendCsv temp/no-artifacts/v02-performance/v02-final-check/V02_PERFORMANCE_RUN_v02-final-check.csv -VbaCsv temp/no-artifacts/v02-vba-comparison/v02-final-check/V02_VBA_COMPARISON_RUN_v02-final-check.csv -NoArtifacts -RunId v02-final-check`
  - Result: passed with overall `warn`.
  - Artifact:
    `temp/no-artifacts/v02-performance-summary/v02-final-check/V02_PERFORMANCE_SUMMARY_v02-final-check.md`
  - Boundary: `warn` is expected because this host used skipped VBA rows rather
    than captured/imported Excel/VBA timings.
- `./scripts/meta-check.ps1 -Fast -NoArtifacts`
  - Result: passed.
  - Coverage: governance, profile scope, language coverage, drift checks,
    `cargo fmt --check`, `cargo clippy`, full workspace `cargo test`, and
    doc tests.

## Checklist

| Item | Status | Evidence |
| --- | --- | --- |
| Benchmark corpus and methodology are published. | Complete | `V02_PERFORMANCE_BENCHMARK_CORPUS_V1.md` and `.csv`. |
| OxVba backend runner emits common markdown/CSV schema. | Complete | `scripts/run-v02-performance.ps1`; final `v02-final-check` artifact. |
| VBA comparison capture/import boundary is executable. | Complete | `scripts/run-v02-vba-comparison.ps1`; skipped-row validation. |
| Threshold policy and trend summary are published. | Complete | `V02_PERFORMANCE_THRESHOLDS_V1.md` and `scripts/summarize-v02-performance.ps1`. |
| Product language avoids absolute speed claims. | Complete | Threshold policy and corpus boundary text. |
| Repo-wide fast validation remains green. | Complete | `./scripts/meta-check.ps1 -Fast -NoArtifacts`. |

## Boundary

The V0.2 claim is a repeatable scaffold and evidence format, not a claim that
OxVba is faster than VBA. Excel/VBA capture is host-conditional; skipped rows
show deterministic boundary behavior only and cannot be used as VBA performance
comparison results.

## Result

`bd-bqm8.11.6` is complete. Parent bead `bd-bqm8.11` is complete because all
child rollout, corpus, runner, VBA harness, threshold, and final-checklist beads
are closed with executable evidence and explicit product boundaries.
