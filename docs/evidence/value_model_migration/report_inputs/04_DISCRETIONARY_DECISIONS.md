# Discretionary Decisions

Status: active

1. Long-string perf workload resize
   - decision:
     reduce the synthetic `long_strings` generator from `512` unit repeats to
     `128` unit repeats
   - evidence basis:
     the original generator overflowed the baseline process stack before
     producing timing data
   - compatibility rationale:
     this changes only the migration perf harness workload shape; it does not
     alter runtime semantics
   - revisit trigger:
     once the perf harness is stable enough to run a heavier long-string corpus
     without stack overflow.
2. String conformance bug classification
   - decision:
     treat `string_slice_ops_dollar.bas` as a repo-wide OxVba bug rather than a
     migration-induced old/new delta
   - evidence basis:
     the same mismatch (`12,45,234` expected vs `0,0,0` actual) reproduced on
     both the fixed baseline and current `HEAD`
   - compatibility rationale:
     this matches the agreed authority hierarchy: VBA/spec/conformance remain
     authoritative over both OxVba versions
   - revisit trigger:
     once the slice semantics are corrected, rerun the string conformance
     bundle and remove this exception.
3. Current canonical string-perf artifact selection
   - decision:
     use `vmd6-perf-check` as the current canonical string perf input
   - evidence basis:
     the repaired one-iteration paired run completed across all five workloads
     and both backends, while the later three-iteration rerun stalled before
     candidate execution
   - compatibility rationale:
     a completed bounded paired run is more trustworthy than a partially
     materialized longer run
   - revisit trigger:
     promote the multi-iteration run once the perf harness reruns cleanly end
     to end.
4. Current canonical Variant-perf artifact selection
   - decision:
     use `vme5-perf-check` as the current canonical Variant perf input
   - evidence basis:
     the one-iteration paired run completed across all selected Variant
     boundary workloads after the new harness landed and smoke-validated
   - compatibility rationale:
     this follows the same bounded-paired-artifact rule used for the string
     lane and avoids claiming perf closure from partial or stale runs
   - revisit trigger:
     promote a multi-iteration Variant perf artifact once the current lane can
     rerun cleanly without materially extending cycle time.
5. Variant-perf workload surface selection
   - decision:
     time host end-to-end exact tests for scalar classification, typed arrays,
     object rebinding, wide-i64 normalization, and variant-matrix results
     rather than a raw microbenchmark-only bridge harness
   - evidence basis:
     `vmm-e5` is specifically about observable old/new migration behavior at
     the COM/Variant boundary, and these exact tests already define the
     correctness contract for that boundary
   - compatibility rationale:
     this keeps the perf artifact aligned with user-visible migration risk
     instead of optimizing an isolated bridge path that may not dominate the
     real execution surface
   - revisit trigger:
     add lower-level bridge microbenchmarks if later optimization work needs
     finer attribution after correctness and matrix closure are secure.
