# V0.2 Performance Thresholds and Trend Policy

Status: active V0.2 threshold policy

Owner bead: `bd-bqm8.11.5`

Machine-readable thresholds: `docs/validation/V02_PERFORMANCE_THRESHOLDS_V1.csv`

## Policy

V0.2 performance thresholds guard the evidence scaffold and product language.
They are not absolute cross-machine speed promises. Results are meaningful only
for the named workload, host class, run command, and artifact set.

## Thresholds

| ID | Area | Metric | Pass | Warn | Fail |
| --- | --- | --- | --- | --- | --- |
| PERF-V02-T001 | Backend runner | Schema | Required columns present | Optional columns missing | Required columns missing |
| PERF-V02-T002 | Backend runner | Rows | At least one backend row | Not applicable | No backend rows |
| PERF-V02-T003 | Backend runner | JIT/VM ratio | Ratio <= 1.25 | Ratio > 1.25 and <= 1.75 | Ratio > 1.75 |
| PERF-V02-T004 | VBA comparison | Status | Captured/imported rows present | Only skipped rows present | No VBA comparison rows |
| PERF-V02-T005 | Product claims | Language | Claims include workload and host boundary | Claims omit some context | Claims assert absolute superiority without evidence |

## Interpretation

- A `warn` summary means the scaffold produced usable evidence but the result
  should not be promoted as a performance claim.
- A `fail` summary means the artifact surface is malformed or too incomplete
  to support V0.2 performance reporting.
- Skipped VBA rows are acceptable on hosts without Excel/VBA automation, but
  they are evidence of boundary handling only.
- Local benchmark ratios are advisory until repeated on a controlled host.
