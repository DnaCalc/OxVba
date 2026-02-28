# Performance Benchmark

- Timestamp (UTC): 2026-02-28T21:33:32Z
- Profile scope: mvp-lang-full-closure-gate-v106
- Iterations: 3
- Workloads: 2
- Aggregate gain percent: 12.53

| Workload | Backend | Baseline ms | Optimized ms | Gain percent |
|---|---|---:|---:|---:|
| conformance_vm | vm | 64056.04 | 67239.81 | -4.97 |
| conformance_jit | jit | 75143.51 | 52577.4 | 30.03 |
