# V0.2 Performance Benchmark Corpus

Date: 2026-04-27

Bead: `bd-bqm8.11.2`

Parent: `bd-bqm8.11`

## Scope

This evidence closes the corpus and methodology bead for the V0.2 performance
scaffold. It defines the workload IDs, engines, optional VBA comparison lane,
artifact schema, iteration policy, and product-language boundaries that later
runner beads must implement.

## Deliverables

- `docs/validation/V02_PERFORMANCE_BENCHMARK_CORPUS_V1.md`
  - Human-readable methodology, corpus table, artifact schema, and boundaries.
- `docs/validation/V02_PERFORMANCE_BENCHMARK_CORPUS_V1.csv`
  - Machine-readable workload rows for runner and checklist traceability.

## Coverage

- Portable OxVba backend rows cover full conformance timing, a focused stable
  string-runtime subset, and project-hosting example timing.
- Existing COM early-binding perf substrate is included as a controlled
  Windows fixture lane.
- VBA comparison is bounded to three simple workload families: scalar loop
  arithmetic, string concatenation/slicing, and array iteration/`ReDim`.

## Boundary

This bead does not add the executable runner or publish performance thresholds.
It deliberately leaves parent `bd-bqm8.11` in-progress. The next ready delivery
bead is `bd-bqm8.11.3`, which must make the OxVba backend runner consume or
match this corpus and emit stable artifacts.

During runner implementation, `V02-PERF-002` was narrowed from a broad
Err/string/financial pattern to explicit stable string workloads because
existing conformance divergences made the broader row non-repeatable.

## Validation

- `./scripts/check-governance.ps1`
  - Result: passed.
- `git diff --check`
  - Result: passed with line-ending normalization warnings only.

## Result

`bd-bqm8.11.2` is complete for corpus and methodology publication only.
