# Performance Benchmark

- Timestamp (UTC): 2026-03-01T15:10:57Z
- Profile scope: mvp-profile-v186
- Iterations: 1
- Workloads: 4
- Aggregate gain percent: 0.94

| Workload | Backend | Baseline ms | Optimized ms | Gain percent |
|---|---|---:|---:|---:|
| conformance_vm | vm | 52671.44 | 50001.55 | 5.07 |
| conformance_jit | jit | 50981.94 | 51672.96 | -1.36 |
| subset_err_string_financial_vm | vm | 7437.36 | 7359.88 | 1.04 |
| subset_err_string_financial_jit | jit | 7113.49 | 7184.45 | -1 |
