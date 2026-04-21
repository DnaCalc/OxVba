# Further Mitigations

Status: active

1. Fix `string_slice_ops_dollar.bas`
   - this is the highest-value correctness follow-up because it currently blocks
     string-family semantic closure on both baseline and migrated `HEAD`.
2. Stabilize the multi-iteration string-perf harness
   - promote a completed three-iteration paired run over the current bounded
     one-iteration canonical artifact.
3. Investigate VM regressions in medium/long/many/code string churn
   - these are the largest current candidate slowdowns in the paired perf data.
4. Investigate JIT regressions for small/medium/many string churn
   - code-string JIT throughput is close to neutral, so the main pressure is in
     the smaller runtime-string families.
5. Look for copy-elision opportunities around `BStr` carrier transitions
   - especially VM/runtime helper paths and COM/native writeback seams that now
     route through the owned carrier.
6. Revisit small-string handling after correctness is fully green
   - the current candidate only beats baseline in VM small-string churn; that
     needs explanation before broader tuning decisions are made.
7. Investigate the Variant scalar-classifier slowdown
   - `vme5-perf-check` currently shows `scalar_classifier` at `+187.84%`,
     which is the largest measured Variant perf regression.
8. Investigate object-result and variant-matrix perf deltas
   - `object_results` is `+31.58%` and `variant_matrix_results` is `+36.72%`
     in the current bounded Variant perf artifact.
9. Revisit hot-path `Variant` footprint once later migration lanes are green
   - `vme5-mem-full` records `Variant = 16 -> 80` bytes, so any later
     performance tuning should assess whether part of that growth can be
     recovered without breaking the new observable contract.
