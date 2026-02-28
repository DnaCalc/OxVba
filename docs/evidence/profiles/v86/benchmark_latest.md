# Performance Benchmark

- Timestamp (UTC): 2026-02-28T14:53:32Z
- Profile scope: mvp-full-typing-conformance-gate-v86
- Iterations: 1
- Workloads: 2
- Aggregate gain percent: -8.49

| Workload | Backend | Baseline ms | Optimized ms | Gain percent |
|---|---|---:|---:|---:|
| conformance_vm | vm | 45057.82 | 52942.78 | -17.5 |
| conformance_jit | jit | 48605.25 | 48358.14 | 0.51 |
