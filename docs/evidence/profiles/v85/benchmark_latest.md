# Performance Benchmark

- Timestamp (UTC): 2026-02-28T14:35:51Z
- Profile scope: mvp-typed-execution-fastpaths-v85
- Iterations: 1
- Workloads: 2
- Aggregate gain percent: 0.31

| Workload | Backend | Baseline ms | Optimized ms | Gain percent |
|---|---|---:|---:|---:|
| conformance_vm | vm | 52923.81 | 52443.38 | 0.91 |
| conformance_jit | jit | 52957.95 | 53110.06 | -0.29 |
