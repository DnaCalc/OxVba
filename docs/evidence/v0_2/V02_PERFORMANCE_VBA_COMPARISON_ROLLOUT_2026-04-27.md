# V0.2 Performance and VBA Comparison Rollout

Date: 2026-04-27

Bead: `bd-bqm8.11.1`

Parent: `bd-bqm8.11`

## Scope

This rollout splits the V0.2 performance scaffold and VBA comparison harness
lane into executable child beads. The parent remains in-progress until the repo
has a reproducible corpus, runnable OxVba backend measurements, a bounded VBA
comparison capture/import path, published thresholds, and a final checklist.

## Child Beads

- `bd-bqm8.11.1`: roll out child beads and current execution boundaries.
- `bd-bqm8.11.2`: publish the benchmark corpus and methodology matrix.
- `bd-bqm8.11.3`: add a reproducible OxVba backend performance runner.
- `bd-bqm8.11.4`: add the bounded VBA comparison capture/import harness.
- `bd-bqm8.11.5`: publish thresholds, trend surfaces, and evidence format.
- `bd-bqm8.11.6`: run the final performance/VBA comparison checklist.

## Existing Substrate

- `scripts/run-bench.ps1` measures conformance and focused VM/JIT subsets with
  baseline-vs-optimized columns and markdown/CSV outputs.
- `scripts/run-com-early-perf.ps1` captures COM early-binding timing evidence.
- Value-model migration perf runners already provide paired old/new string and
  Variant timing examples under `docs/evidence/value_model_migration/runs/`.
- Historical profile evidence already stores benchmark artifacts under
  `docs/evidence/profiles/`.

## Boundary

This lane will not claim stable absolute performance numbers from a shared local
machine. The V0.2 claim is a repeatable scaffold: named workloads, consistent
output schema, backend comparison rows, bounded VBA oracle capture/import when
available, and explicit thresholds for trend interpretation.

## Result

`bd-bqm8.11.1` is complete as a support rollout bead. Parent `bd-bqm8.11`
remains in-progress; the next delivery bead is `bd-bqm8.11.2`.
