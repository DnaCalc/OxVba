# Value Model Migration Final Report

Date: 2026-04-22
Status: final
Baseline tag: `pre-value-model-migration-2026-04-20`
Baseline commit: `dd1c295b2a3d3a1530dd034d9bb4a6b4c38ea57a`
Candidate commit for the paired evidence set: `834d580c4eef4b1df88c4a5919a6ffc88d603b71`

## 12.1 Executive result

The Windows VBA 7.1 x64 value-model migration is complete. The migrated value
model is now the active implementation, the selected paired correctness matrix
is green across the required migration bundles, and no migration-specific
rollout blocker remains.

The remaining known semantic divergence in the evidence set is
`string_slice_ops_dollar.bas`. It fails on both the fixed baseline and the
migrated head and is therefore classified as a pre-existing OxVba bug rather
than a migration regression.

The canonical artifact surfaces for this decision are:

1. [PAIRED_RESULT_INDEX_2026-04-22.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/PAIRED_RESULT_INDEX_2026-04-22.md)
2. [LATEST_ARTIFACT_MAP.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/LATEST_ARTIFACT_MAP.csv)

## 12.2 Representation summary

The runtime string carrier is now an owned `BStr` substrate with Windows-style
UTF-16 storage semantics rather than a thin wrapper over Rust `String`. String
pointer-sensitive behavior at `StrPtr` and `VarPtr(String)` is now much closer
to a direct consequence of the canonical runtime carrier.

The runtime value carrier now uses an owned semantic `Variant` over a
Windows-shaped `VariantCore`, rather than the earlier bounded 16-byte bridge
that rejected important runtime shapes. Strings, arrays, objects, and boundary
materialization now live within one canonical value-carrier design instead of
being handled as a side bridge around the real runtime substrate.

Object identity has moved from integer `ObjectHandle` tokens to `ObjectRef`,
backed by an `IUnknown`-style runtime identity and refcount substrate. COM
identity and lifetime are anchored on retained `IUnknown` truth, while the
runtime now carries object identity through `ObjectRef` instead of token-only
maps. Event payload transport was widened onto the migrated semantic carrier,
with callback-object identity preserved through retained `ObjectRef` values.

Pointer-helper and ABI-sensitive surfaces now reflect the migrated substrate:
`ObjPtr` is `IUnknown`-backed, and `VarPtr(Variant)` now supports object and
array payload materialization via real `VT_UNKNOWN` and `VT_ARRAY | VT_VARIANT`
containers. Broad native struct-overlay parity and unconstrained UDT-byref ABI
parity remain explicitly bounded outside this closed migration scope.

## 12.3 Correctness result

The selected paired correctness matrix contains `36` rows:

1. `6` string / BSTR boundary rows
2. `16` Variant / VARIANT boundary rows
3. `7` interface / event rows
4. `7` ABI / layout-sensitive rows

All selected rows pass on both the fixed baseline and the migrated candidate.
No migration-specific correctness regression remains in the selected matrix.

Canonical correctness artifacts:

1. string boundary:
   [vmd6-corr-boundary-final](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmd6-corr-boundary-final/comparison/correctness_summary.md>)
2. Variant boundary:
   [vme5-corr-boundary-final](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vme5-corr-boundary-final/comparison/correctness_summary.md>)
3. interface and event:
   [vmf6-interface-event-matrix-r3](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmf6-interface-event-matrix-r3/comparison/correctness_summary.md>)
4. ABI and layout:
   [vmg5-abi-layout-r3](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmg5-abi-layout-r3/comparison/correctness_summary.md>)

Authority-hierarchy classification:

1. Excel/VBA on Windows remains the top oracle
2. published specs remain the next authority
3. old OxVba remains the regression anchor only
4. `string_slice_ops_dollar.bas` is a confirmed old/new shared bug, not a
   migration divergence
5. broad native struct-overlay and unconstrained UDT-byref ABI remain bounded
   outside this migration matrix rather than unresolved inside it.

## 12.4 Discretionary decisions

The migration includes retained discretionary choices that should remain
visible and revisitable:

1. object identity uses `ObjectRef` over an `IUnknown`-style runtime base,
   rather than exposing raw interface pointers throughout the runtime
2. native COM truth is retained through an `IUnknown`-anchored wrapper rather
   than flattening the whole runtime into raw pointer state
3. the canonical runtime `Variant` is an owned semantic carrier over
   `VariantCore`, not literally a borrowed process `VARIANT` at every point
4. pointer-helper object and array materialization were kept as honest
   boundary-visible projections where that remained the truthful design
5. the canonical final artifact set is the selected paired matrix and bounded
   perf/memory artifacts listed in the report input index.

Full decision register:
[04_DISCRETIONARY_DECISIONS.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/04_DISCRETIONARY_DECISIONS.md)

## 12.5 Performance and memory result

Canonical performance and memory artifacts:

1. string perf:
   [vmd6-perf-check](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_string_perf_vmd6-perf-check/comparison/string_perf_summary.md>)
2. Variant perf:
   [vme5-perf-check](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_variant_perf_vme5-perf-check/comparison/variant_perf_summary.md>)
3. string and Variant memory:
   [vmd6-mem-full](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_memory_vmd6-mem-full/layout_metrics_summary.csv>)
   and
   [vme5-mem-full](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_memory_vme5-mem-full/layout_metrics_summary.csv>)
4. identity/layout deltas:
   [vmf2-mem-identity-smoke](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_memory_vmf2-mem-identity-smoke/comparison/layout_metrics.csv>)

String timing summary:

1. VM:
   - `small_strings -16.81%`
   - `medium_strings +75.54%`
   - `long_strings +67.06%`
   - `many_strings +83.84%`
   - `code_strings +48.92%`
2. JIT:
   - `small_strings +27.66%`
   - `medium_strings +51.84%`
   - `long_strings +17.03%`
   - `many_strings +44.85%`
   - `code_strings +0.2%`

Variant timing summary:

1. `scalar_classifier +187.84%`
2. `numeric_classifier +13.79%`
3. `typed_array_results -6.59%`
4. `typed_decimal_array_results -4.94%`
5. `object_results +31.58%`
6. `wide_i64_array_boundary +18.8%`
7. `variant_matrix_results +36.72%`

Interpretation:

1. the current string perf artifact is directional rather than a final stable
   throughput claim because it is still the repaired paired one-iteration run
2. the current Variant perf artifact is bounded but adequate for mitigation
   planning
3. the candidate improves VM small-string churn and two typed-array Variant
   rows, but most string and several Variant rows are slower and remain part of
   the mitigation backlog.

Memory and layout result:

1. string-focused carrier sizes stayed stable in the current probes
2. the accepted representation-growth deltas introduced by the full migration
   are:
   - `Variant 16 -> 80`
   - `ObjectIdentityCarrier 4/4 -> 8/8`
   - `ComCallbackPayload 40 -> 48`
3. `RuntimeValue` and `ComValue` remained unchanged in the identity-smoke lane
   at `64/8`
4. the measured Variant-heavy working-set deltas remained modest in the paired
   memory artifact despite carrier growth.

## 12.6 Further mitigations

Further work is now optimization and bounded follow-on work, not migration
completion work:

1. fix `string_slice_ops_dollar.bas`
2. stabilize multi-iteration string perf harnessing
3. investigate the current VM/JIT string regressions
4. investigate Variant perf hotspots, especially scalar classification
5. pursue BSTR and Variant copy-elision opportunities where they do not change
   correctness
6. revisit `Variant` and callback footprint only if later evidence shows the
   current sizes are materially costly
7. if scope expands beyond this migration, run a separate project for broad
   native UDT-byref and struct-overlay parity.

Canonical mitigation register:
[06_FURTHER_MITIGATIONS.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/06_FURTHER_MITIGATIONS.md)
