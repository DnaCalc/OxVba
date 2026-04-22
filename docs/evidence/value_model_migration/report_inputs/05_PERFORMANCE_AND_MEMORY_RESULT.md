# Performance And Memory Result

Status: final

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

Variant timing summary:

1. `scalar_classifier`: candidate slower by `+187.84%`
2. `numeric_classifier`: candidate slower by `+13.79%`
3. `typed_array_results`: candidate faster by `-6.59%`
4. `typed_decimal_array_results`: candidate faster by `-4.94%`
5. `object_results`: candidate slower by `+31.58%`
6. `wide_i64_array_boundary`: candidate slower by `+18.8%`
7. `variant_matrix_results`: candidate slower by `+36.72%`

Timing interpretation:

1. the canonical string-perf artifact is still a repaired one-iteration paired
   run, so it should be treated as directional evidence rather than a final
   stable throughput claim
2. the canonical Variant-perf artifact is a bounded paired run that is strong
   enough for attribution and mitigation planning but not yet a continuous perf
   gate
3. string timing currently shows one clear improvement in VM small-string churn
   and broad slowdown elsewhere, especially VM medium/long/many-string paths
4. JIT code-string throughput is effectively neutral in the current artifact
5. Variant timing is mixed:
   - typed array and typed decimal-array rows improved slightly
   - scalar classification is the largest current regression
   - object, matrix, and wide-i64 rows remain slower and are retained in the
     mitigation backlog.

Memory summary:

1. observed carrier/layout sizes and alignments did not change between baseline
   and candidate in the string-focused memory probe:
   - `BStr = 24`
   - `RuntimeValue = 64`
   - `SafeArray = 64`
   - `ComValue = 64`
   - `ComInvokeArg = 88`
2. working-set deltas were small in the current paired run:
   - `cli_small_strings`: `+65536` bytes
   - `cli_many_strings`: `+36864` bytes
   - `cli_code_strings`: `-57344` bytes
   - `com_variant_bstr_array`: `+53248` bytes
3. the post-`ObjectRef` memory probe records the accepted representation-growth
   deltas introduced by the full migration:
   - `Variant = 16 -> 80` bytes
   - `ObjectIdentityCarrier = 4/4 -> 8/8`
   - `ComCallbackPayload = 40 -> 48` bytes
4. `RuntimeValue` and `ComValue` remain unchanged in the identity-smoke probe:
   - `RuntimeValue = 64/8`
   - `ComValue = 64/8`
5. the Variant-heavy paired working-set rows in `vme5-mem-full` remain modest
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
   paired pointer-snapshot artifact after the new canonical carrier landed
4. the authoritative identity/layout delta source for the final report is:
   `docs/evidence/value_model_migration/runs/value_model_memory_vmf2-mem-identity-smoke/comparison/layout_metrics.csv`.
