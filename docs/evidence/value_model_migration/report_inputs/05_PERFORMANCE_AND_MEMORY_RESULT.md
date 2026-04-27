# Performance And Memory Result

Status: final bounded artifact set

Canonical artifacts:

1. string perf:
   - run id: `vmd7-perf-bstr-coreonly`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_string_perf_vmd7-perf-bstr-coreonly/string_perf_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_string_perf_vmd7-perf-bstr-coreonly/comparison/string_perf_summary.csv`
2. memory:
   - run id: `vmd7-mem-bstr-coreonly`
   - layout summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vmd7-mem-bstr-coreonly/layout_metrics_summary.csv`
   - process summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vmd7-mem-bstr-coreonly/process_memory_summary.csv`
   - pointer snapshot summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vmd7-mem-bstr-coreonly/pointer_snapshot_summary.csv`
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
   - `small_strings`: candidate faster by `-6.21%`
   - `long_strings`: candidate slower by `+8.39%`
   - `many_strings`: candidate faster by `-4.97%`
   - `code_strings`: candidate faster by `-17.76%`
2. JIT:
   - `small_strings`: candidate slower by `+3.83%`
   - `long_strings`: candidate slower by `+52.18%`
   - `many_strings`: candidate near-neutral at `+0.07%`
   - `code_strings`: candidate faster by `-26.62%`

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
   run family, but `vmd7-perf-bstr-coreonly` is now the current bounded paired
   artifact for the core-only `BStr` carrier
2. the canonical Variant-perf artifact is a bounded paired run that is strong
   enough for attribution and mitigation planning but not yet a continuous perf
   gate
3. the core-only `BStr` carrier materially improved VM `code_strings` and JIT
   `code_strings`, and kept VM `small_strings` / `many_strings` positive
4. the main remaining string-timing regression in the refreshed artifact is JIT
   `long_strings` at `+52.18%`; JIT `small_strings` is a mild regression
5. Variant timing is mixed:
   - typed array and typed decimal-array rows improved slightly
   - scalar classification is the largest current regression
   - object, matrix, and wide-i64 rows remain slower and are retained in the
     mitigation backlog.

Memory summary:

1. the refreshed string-focused memory probe shows the core-only `BStr`
   carrier removed the temporary size inflation introduced by the staged
   compatibility-cache attempt:
   - `BStr = 24`
   - `RuntimeValue = 64`
   - `SafeArray = 64`
   - `ComValue = 64`
   - `ComInvokeArg = 88`
2. the same refreshed probe confirms the string lane is no longer carrying the
   `BStr = 56` intermediate state that forced the extra delivery slice
3. working-set deltas were small in the current paired run:
   - `cli_small_strings`: `+65536` bytes
   - `cli_many_strings`: `+36864` bytes
   - `cli_code_strings`: `-57344` bytes
   - `com_variant_bstr_array`: `+53248` bytes
4. the post-`ObjectRef` memory probe records the accepted representation-growth
   deltas introduced by the full migration:
   - `Variant = 16 -> 80` bytes
   - `ObjectIdentityCarrier = 4/4 -> 8/8`
   - `ComCallbackPayload = 40 -> 48` bytes
5. `RuntimeValue` and `ComValue` remain unchanged in the identity-smoke probe:
   - `RuntimeValue = 64/8`
   - `ComValue = 64/8`
6. the Variant-heavy paired working-set rows in `vme5-mem-full` remain modest
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
