# Correctness Result

Status: final

Selected matrix result:

1. the migration closes with `36` paired correctness lanes in the selected
   matrix
2. breakdown:
   - `6` string / BSTR boundary lanes
   - `16` Variant / VARIANT boundary lanes
   - `7` interface / event lanes
   - `7` ABI / layout-sensitive lanes
3. baseline and candidate both pass every selected paired lane
4. no migration-specific correctness regression remains in the selected matrix.

Canonical paired correctness artifacts:

1. string boundary bundle: `vmd6-corr-boundary-final`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmd6-corr-boundary-final/correctness_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmd6-corr-boundary-final/comparison/correctness_summary.md`
2. Variant boundary bundle: `vme5-corr-boundary-final`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vme5-corr-boundary-final/correctness_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vme5-corr-boundary-final/comparison/correctness_summary.md`
3. interface/event bundle: `vmf6-interface-event-matrix-r3`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmf6-interface-event-matrix-r3/correctness_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmf6-interface-event-matrix-r3/comparison/correctness_summary.md`
4. ABI/layout bundle: `vmg5-abi-layout-r3`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmg5-abi-layout-r3/correctness_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmg5-abi-layout-r3/comparison/correctness_summary.md`

Authority-hierarchy classification:

1. the selected old/new matrix is green across the migrated surface
2. the selected matrix therefore confirms no migration-induced divergence
   against the fixed baseline in the covered lanes
3. correctness authority still remains:
   - Excel/VBA on Windows
   - published specs
   - old OxVba
4. `string_slice_ops_dollar.bas` remains a confirmed old/new shared bug:
   - expected slots: `12,45,234`
   - observed slots on both baseline and candidate: `0,0,0`
   - classification: pre-existing OxVba semantic bug, not a migration
     regression
5. broad native struct-overlay parity and unconstrained UDT-byref native ABI
   parity remain explicitly bounded outside the selected migration matrix
   instead of being misclassified as open migration regressions.

See also:

1. [PAIRED_RESULT_INDEX_2026-04-22.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/PAIRED_RESULT_INDEX_2026-04-22.md)
2. [LATEST_ARTIFACT_MAP.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/LATEST_ARTIFACT_MAP.csv)
