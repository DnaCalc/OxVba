# Performance Benchmark

- Timestamp (UTC): 2026-03-01T14:14:54Z
- Profile scope: mvp-profile-v172
- Iterations: 3
- Workloads: 4
- Aggregate gain percent: 3.35

| Workload | Backend | Baseline ms | Optimized ms | Gain percent |
|---|---|---:|---:|---:|
| conformance_vm | vm | 57107.46 | 54342.37 | 4.84 |
| conformance_jit | jit | 52421.05 | 51857.67 | 1.07 |
| subset_err_string_financial_vm | vm | 7521.96 | 7542.3 | -0.27 |
| subset_err_string_financial_jit | jit | 7632.66 | 7039.52 | 7.77 |
