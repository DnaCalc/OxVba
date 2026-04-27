# V0.2 Performance Benchmark Corpus and Methodology

Status: active V0.2 methodology

Owner bead: `bd-bqm8.11.2`

Machine-readable corpus: `docs/validation/V02_PERFORMANCE_BENCHMARK_CORPUS_V1.csv`

## Purpose

The V0.2 performance lane measures repeatable workload families rather than
publishing anecdotal speed claims. Every workload must name its source command
or source document, the engines being compared, the primary metric, iteration
policy, and the claim boundary.

## Corpus

| ID | Area | Workload | Engines | VBA Comparison | Boundary |
| --- | --- | --- | --- | --- | --- |
| V02-PERF-001 | Conformance | Full conformance suite | VM, JIT | No | Descriptive whole-suite timing only. |
| V02-PERF-002 | Conformance | Stable string runtime subset | VM, JIT | No | Portable string runtime subset timing; known divergent `string_slice_ops_dollar.bas` is excluded. |
| V02-PERF-003 | Project hosting | Project hosting examples | VM, JIT | No | Startup/hosting scaffold timing only. |
| V02-PERF-004 | COM early binding | Compile and runtime early-bind fixture | Compiler, VM, JIT | No | Controlled Windows COM fixture timing. |
| V02-PERF-005 | VBA compare | Scalar loop arithmetic | VM, JIT, optional Excel/VBA | Optional | Comparative oracle observation when VBA capture is available. |
| V02-PERF-006 | VBA compare | String concatenation and slicing | VM, JIT, optional Excel/VBA | Optional | Comparative oracle observation when VBA capture is available. |
| V02-PERF-007 | VBA compare | Array iteration and `ReDim` | VM, JIT, optional Excel/VBA | Optional | Comparative oracle observation when VBA capture is available. |

## Methodology

- Use elapsed wall-clock milliseconds as the V0.2 primary metric.
- Run at least one warmup before measured iterations when a runner supports it.
- Use at least three measured iterations for portable OxVba backend rows and at
  least five measured iterations for COM/VBA-adjacent rows.
- Emit both markdown and CSV artifacts. CSV is authoritative for automated
  trend checks; markdown is the review surface.
- Include run ID, timestamp, host OS, command or workload source, backend,
  iteration count, mean, min, max, and claim boundary in generated artifacts.
- Do not compare absolute numbers across unrelated machines as a product claim.
  Treat local timing as trend evidence for the same workload and host class.

## Artifact Schema

Runners in this lane should converge on these CSV columns where practical:

```text
run_id,timestamp_utc,host_os,workload_id,workload,engine,mode,iterations,warmup_iterations,mean_ms,min_ms,max_ms,comparison_baseline,ratio,claim_boundary
```

Runner-specific columns are allowed, but the common columns above are the V0.2
contract for trend and checklist evidence.

## VBA Boundary

Excel/VBA comparison rows are optional because they require a Windows host with
Excel automation available. A runner may emit `skipped` rows for VBA capture
when the host is unavailable; that is valid evidence for portability, but it is
not a VBA performance comparison result.

## Product Language

Allowed V0.2 language:

- "OxVba has a reproducible benchmark corpus and artifact schema for VM/JIT and
  selected host-facing workloads."
- "VBA comparison rows are captured when a Windows Excel/VBA host is available."
- "Performance trends are interpreted with explicit noise and host boundaries."

Disallowed V0.2 language:

- "OxVba is faster than VBA" without a named workload, host, artifact, and
  threshold result.
- "Local benchmark numbers are stable absolute product performance."
- "Skipped VBA capture rows prove parity or superiority."
