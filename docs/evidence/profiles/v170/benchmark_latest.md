# Performance Benchmark

- Timestamp (UTC): 2026-03-01T13:51:38Z
- Profile scope: mvp-profile-v170
- Iterations: 3
- Workloads: 4
- Aggregate gain percent: 1.86

| Workload | Backend | Baseline ms | Optimized ms | Gain percent |
|---|---|---:|---:|---:|
| conformance_vm | vm | 54892.01 | 53575.32 | 2.4 |
| conformance_jit | jit | 52744.21 | 54644.98 | -3.6 |
| subset_err_string_financial_vm | vm | 7025.41 | 7270.33 | -3.49 |
| subset_err_string_financial_jit | jit | 8214.9 | 7216.79 | 12.15 |
