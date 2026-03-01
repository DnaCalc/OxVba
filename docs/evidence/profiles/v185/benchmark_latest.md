# Performance Benchmark

- Timestamp (UTC): 2026-03-01T15:03:10Z
- Profile scope: mvp-profile-v185
- Iterations: 1
- Workloads: 4
- Aggregate gain percent: 3.32

| Workload | Backend | Baseline ms | Optimized ms | Gain percent |
|---|---|---:|---:|---:|
| conformance_vm | vm | 58256.35 | 55128.1 | 5.37 |
| conformance_jit | jit | 53595.21 | 52686.16 | 1.7 |
| subset_err_string_financial_vm | vm | 7494.99 | 7285.06 | 2.8 |
| subset_err_string_financial_jit | jit | 7879.58 | 7610.08 | 3.42 |
