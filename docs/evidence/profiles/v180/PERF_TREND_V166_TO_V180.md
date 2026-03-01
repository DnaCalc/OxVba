# PERF_TREND_V166_TO_V180.md

- Timestamp (UTC): 2026-03-01T14:52:36Z
- Profile: v180
- Inputs:
  - docs/evidence/profiles/v166/benchmark_latest.csv
  - docs/evidence/profiles/v180/benchmark_latest.csv

## Common workload trend

| Workload | v166 gain % | v180 gain % | Delta (v180-v166) |
|---|---:|---:|---:|
| conformance_vm | -3.26 | 2.77 | 6.03 |
| conformance_jit | 10.8 | -3.2 | -14 |

## Notes
- v180 adds focused subset workloads (`subset_err_string_financial_vm/jit`) that were not present in v166 baseline.
- v180 aggregate gain (all 4 workloads): 0.45% (from `docs/evidence/profiles/v180/benchmark_latest.md`).
- This trend is descriptive only; benchmark noise is expected on shared local hardware.
