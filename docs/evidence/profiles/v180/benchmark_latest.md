# Performance Benchmark

- Timestamp (UTC): 2026-03-01T14:52:06Z
- Profile scope: mvp-profile-v180
- Iterations: 1
- Workloads: 4
- Aggregate gain percent: 0.45

| Workload | Backend | Baseline ms | Optimized ms | Gain percent |
|---|---|---:|---:|---:|
| conformance_vm | vm | 52633.89 | 51175.87 | 2.77 |
| conformance_jit | jit | 49929.47 | 51525.57 | -3.2 |
| subset_err_string_financial_vm | vm | 7329.87 | 6922.41 | 5.56 |
| subset_err_string_financial_jit | jit | 7030.69 | 7264.87 | -3.33 |
