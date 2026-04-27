# Value Model Migration Final Report

Date: 2026-04-27
Status: final
Baseline tag: `pre-value-model-migration-2026-04-20`
Baseline commit: `dd1c295b2a3d3a1530dd034d9bb4a6b4c38ea57a`
Candidate commit for the paired evidence set: `834d580c4eef4b1df88c4a5919a6ffc88d603b71`

## 12.1 Executive result

The migrated value model is now the active implementation for the scoped
migration. The selected old/new matrix is green across the required correctness
bundles for string / `BSTR`, Variant / `VARIANT`, interface identity, COM event
transport, and ABI/layout-sensitive rows.

No migration-specific correctness regression remains in the selected matrix.
The remaining known semantic divergence in the evidence set is
`string_slice_ops_dollar.bas`; it fails on both the fixed baseline and migrated
head and is therefore classified as a pre-existing OxVba bug rather than a
migration regression.

Broad native struct-overlay parity and unconstrained UDT-byref native ABI parity
remain explicitly bounded outside this migration. That is a scoped closure
boundary, not a hidden parity claim.

Canonical report inputs:

1. [REPORT_INPUT_INDEX.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/REPORT_INPUT_INDEX.md)
2. [PAIRED_RESULT_INDEX_2026-04-22.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/PAIRED_RESULT_INDEX_2026-04-22.md)
3. [LATEST_ARTIFACT_MAP.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/LATEST_ARTIFACT_MAP.csv)

## 12.2 Representation summary

The runtime string carrier is now an owned `BStr` substrate with BSTR-compatible
UTF-16 storage semantics. Windows builds allocate, clone, measure, and free
through BSTR APIs; non-Windows builds keep the same emulated layout.

The runtime `Variant` / `SAFEARRAY` family has been migrated to retained
canonical carriers for the scoped runtime value lanes. The remaining
`RuntimeValue` surfaces are classified as compatibility contracts rather than
internal value storage.

Object identity has moved from integer `ObjectHandle` tokens to `ObjectRef`,
backed by an `IUnknown`-style runtime identity and refcount substrate. Event
payload transport now preserves callback-object identity through retained
`ObjectRef` values.

Pointer-helper and ABI-sensitive surfaces reflect the migrated substrate:
`ObjPtr` is `IUnknown`-backed, and `VarPtr(Variant)` supports object and array
payload materialization via `VT_UNKNOWN` and `VT_ARRAY | VT_VARIANT`
containers.

The struct / UDT / native-layout lane is closed only for the narrowed migration
scope: the bounded non-boundary UDT subset plus selected pointer-helper and
native-writeback ABI-sensitive rows. Broad native struct-overlay parity,
unconstrained UDT-byref native ABI parity, and arbitrary native packing/alignment
parity remain bounded outside this migration.

## 12.3 Correctness result

The selected paired correctness matrix contains `36` rows:

1. `6` string / BSTR boundary rows
2. `16` Variant / VARIANT boundary rows
3. `7` interface / event rows
4. `7` ABI / layout-sensitive rows

All selected rows pass on both the fixed baseline and the migrated candidate.

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

1. Excel/VBA on Windows remains the top oracle.
2. Published specs remain the next authority.
3. Old OxVba remains the regression anchor only.
4. `string_slice_ops_dollar.bas` is a confirmed old/new shared bug, not a
   migration divergence.
5. Broad native struct-overlay and unconstrained UDT-byref ABI remain bounded
   outside this migration matrix.

## 12.4 Discretionary decisions

The migration retains the following visible decisions:

1. canonical runtime object identity uses `ObjectRef` over an
   `IUnknown`-style runtime base
2. native COM identity remains retained-wrapper state anchored on `IUnknown`
3. canonical runtime `Variant` keeps owned side data around the Windows-shaped
   core where needed for real semantic payloads
4. carrier growth is accepted where it buys honest boundary behavior
5. pointer helpers project real boundary shapes rather than raw internal storage
6. `VarPtr(Variant)` now supports object and array container materialization
7. broad native struct-overlay and unconstrained UDT-byref ABI parity are
   excluded from this migration unless a later workset expands scope.

Full decision register:
[04_DISCRETIONARY_DECISIONS.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/04_DISCRETIONARY_DECISIONS.md)

## 12.5 Performance and memory result

Canonical performance and memory artifacts:

1. string perf:
   [vmd7-perf-bstr-coreonly](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_string_perf_vmd7-perf-bstr-coreonly/comparison/string_perf_summary.csv>)
2. string memory:
   [vmd7-mem-bstr-coreonly](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_memory_vmd7-mem-bstr-coreonly/layout_metrics_summary.csv>)
3. Variant perf:
   [vme5-perf-check](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_variant_perf_vme5-perf-check/comparison/variant_perf_summary.csv>)
4. Variant memory:
   [vme5-mem-full](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_memory_vme5-mem-full/layout_metrics_summary.csv>)
5. identity/layout deltas:
   [vmf2-mem-identity-smoke](</C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_memory_vmf2-mem-identity-smoke/comparison/layout_metrics.csv>)

String timing summary:

1. VM:
   - `small_strings -6.21%`
   - `long_strings +8.39%`
   - `many_strings -4.97%`
   - `code_strings -17.76%`
2. JIT:
   - `small_strings +3.83%`
   - `long_strings +52.18%`
   - `many_strings +0.07%`
   - `code_strings -26.62%`

Variant timing summary:

1. `scalar_classifier +187.84%`
2. `numeric_classifier +13.79%`
3. `typed_array_results -6.59%`
4. `typed_decimal_array_results -4.94%`
5. `object_results +31.58%`
6. `wide_i64_array_boundary +18.8%`
7. `variant_matrix_results +36.72%`

Interpretation:

1. the current string and Variant perf artifacts are bounded paired runs,
   suitable for mitigation planning but not yet continuous perf gates
2. the core-only `BStr` carrier improved VM `small_strings`, VM `many_strings`,
   VM `code_strings`, and JIT `code_strings`
3. the largest current string regression is JIT `long_strings`
4. Variant timing remains mixed, with scalar classification the largest
   current regression
5. accepted representation-growth deltas are recorded explicitly:
   - `Variant 16 -> 80`
   - `ObjectIdentityCarrier 4/4 -> 8/8`
   - `ComCallbackPayload 40 -> 48`

## 12.6 Further mitigations

Further work is optimization and bounded follow-on work, not migration
completion work:

1. fix `string_slice_ops_dollar.bas`
2. stabilize multi-iteration string perf harnessing
3. investigate the JIT `long_strings` and mild JIT `small_strings` regressions
4. investigate Variant perf hotspots, especially scalar classification
5. pursue BSTR and Variant copy-elision opportunities where they do not change
   correctness
6. revisit `Variant` and callback footprint only if later evidence shows the
   current sizes are materially costly
7. if broader native parity becomes in-scope later, open a dedicated follow-on
   for native UDT-byref and struct-overlay closure.

Canonical mitigation register:
[06_FURTHER_MITIGATIONS.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/06_FURTHER_MITIGATIONS.md)
