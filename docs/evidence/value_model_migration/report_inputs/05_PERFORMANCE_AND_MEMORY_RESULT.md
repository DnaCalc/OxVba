# Performance And Memory Result

Status: active

Canonical artifacts:

1. string perf:
   - run id: `vmd6-perf-check`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_string_perf_vmd6-perf-check/string_perf_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_string_perf_vmd6-perf-check/comparison/string_perf_summary.csv`
2. memory:
   - run id: `vmd6-mem-full`
   - layout summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vmd6-mem-full/layout_metrics_summary.csv`
   - process summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vmd6-mem-full/process_memory_summary.csv`
   - pointer snapshot summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vmd6-mem-full/pointer_snapshot_summary.csv`
3. Variant perf:
   - run id: `vme5-perf-check`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_variant_perf_vme5-perf-check/variant_perf_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_variant_perf_vme5-perf-check/comparison/variant_perf_summary.csv`
4. Variant memory:
   - run id: `vme5-mem-full`
   - layout summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vme5-mem-full/layout_metrics_summary.csv`
   - process summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vme5-mem-full/process_memory_summary.csv`
   - pointer snapshot summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vme5-mem-full/pointer_snapshot_summary.csv`

Timing summary:

1. VM:
   - `small_strings`: candidate faster by `-16.81%`
   - `medium_strings`: candidate slower by `+75.54%`
   - `long_strings`: candidate slower by `+67.06%`
   - `many_strings`: candidate slower by `+83.84%`
   - `code_strings`: candidate slower by `+48.92%`
2. JIT:
   - `small_strings`: candidate slower by `+27.66%`
   - `medium_strings`: candidate slower by `+51.84%`
   - `long_strings`: candidate slower by `+17.03%`
   - `many_strings`: candidate slower by `+44.85%`
   - `code_strings`: candidate near-neutral at `+0.2%`

Timing interpretation:

1. the current paired string-perf signal is directional, not final, because the
   canonical artifact is the repaired one-iteration run
2. the resized long-string workload is now executable on both baseline and
   candidate, which was not true for the original oversized generator
3. the current candidate shows broad slowdown outside VM small-string churn and
   near-neutral JIT code-string throughput.
4. the current bounded Variant-perf signal is mixed rather than uniformly
   regressive
5. the largest current Variant slowdown is `scalar_classifier` at `+187.84%`
6. `typed_array_results` and `typed_decimal_array_results` currently trend
   slightly faster on candidate at `-6.59%` and `-4.94%`
7. object rebinding, variant-matrix materialization, and wide-i64 boundary rows
   currently show positive candidate deltas and need later attribution.

Memory summary:

1. observed carrier/layout sizes and alignments did not change between baseline
   and candidate in the current memory probe:
   - `BStr = 24`
   - `RuntimeValue = 64`
   - `Variant = 16`
   - `SafeArray = 64`
   - `ComValue = 64`
   - `ComInvokeArg = 88`
   - `ComCallbackPayload = 40`
2. working-set deltas were small in the current paired run:
   - `cli_small_strings`: `+65536` bytes
   - `cli_many_strings`: `+36864` bytes
   - `cli_code_strings`: `-57344` bytes
   - `com_variant_bstr_array`: `+53248` bytes
3. the Variant migration introduces the first large carrier-layout change seen
   in the current probes:
   - `Variant = 16 -> 80` bytes
4. the Variant-heavy paired working-set rows in `vme5-mem-full` remain modest
   despite that carrier growth:
   - `com_variant_bstr_array`: `-53248` bytes
   - `com_variant_wide_i64_array_boundary`: `-106496` bytes
   - `com_variant_decimal_array`: `+61440` bytes
   - `com_variant_object_result`: `+155648` bytes
   - `com_variant_matrix_result`: `+53248` bytes

Boundary-relevant observations:

1. `com_variant_bstr_array` is included in the memory lane and currently shows a
   small positive candidate working-set delta
2. pointer snapshot logs for `StrPtr`, `VarPtr(String)`, and
   `VarPtr(Variant)` were captured in the same paired memory run.
3. the Variant-specific memory lane confirms that `VarPtr(Variant)` still has a
   paired pointer-snapshot artifact after the new canonical carrier landed.
