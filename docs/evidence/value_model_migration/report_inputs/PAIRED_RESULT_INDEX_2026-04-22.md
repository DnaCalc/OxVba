# Paired Result Index

Date: `2026-04-22`

This file is the canonical grouped index for the selected old/new artifacts used
by the final value-model migration report.

## Correctness

1. String boundary bundle
   - run id: `vmd6-corr-boundary-final`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmd6-corr-boundary-final/correctness_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmd6-corr-boundary-final/comparison/correctness_summary.md`
2. Variant boundary bundle
   - run id: `vme5-corr-boundary-final`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vme5-corr-boundary-final/correctness_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vme5-corr-boundary-final/comparison/correctness_summary.md`
3. Interface/event bundle
   - run id: `vmf6-interface-event-matrix-r3`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmf6-interface-event-matrix-r3/correctness_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmf6-interface-event-matrix-r3/comparison/correctness_summary.md`
4. ABI/layout bundle
   - run id: `vmg5-abi-layout-r3`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmg5-abi-layout-r3/correctness_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_correctness_vmg5-abi-layout-r3/comparison/correctness_summary.md`

## Performance

1. String performance bundle
   - run id: `vmd6-perf-check`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_string_perf_vmd6-perf-check/string_perf_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_string_perf_vmd6-perf-check/comparison/string_perf_summary.md`
2. Variant performance bundle
   - run id: `vme5-perf-check`
   - summary:
     `docs/evidence/value_model_migration/runs/value_model_variant_perf_vme5-perf-check/variant_perf_summary.csv`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_variant_perf_vme5-perf-check/comparison/variant_perf_summary.md`

## Memory And Layout

1. String memory/layout bundle
   - run id: `vmd6-mem-full`
   - layout summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vmd6-mem-full/layout_metrics_summary.csv`
   - process summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vmd6-mem-full/process_memory_summary.csv`
   - pointer snapshot summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vmd6-mem-full/pointer_snapshot_summary.csv`
2. Variant memory/layout bundle
   - run id: `vme5-mem-full`
   - layout summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vme5-mem-full/layout_metrics_summary.csv`
   - process summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vme5-mem-full/process_memory_summary.csv`
   - pointer snapshot summary:
     `docs/evidence/value_model_migration/runs/value_model_memory_vme5-mem-full/pointer_snapshot_summary.csv`
3. Object-identity layout delta bundle
   - run id: `vmf2-mem-identity-smoke`
   - comparison:
     `docs/evidence/value_model_migration/runs/value_model_memory_vmf2-mem-identity-smoke/comparison/layout_metrics.csv`

## Supporting Classification Notes

1. `docs/evidence/value_model_migration/INTERFACE_AND_EVENT_MATRIX_2026-04-22.md`
2. `docs/evidence/value_model_migration/ABI_LAYOUT_MATRIX_2026-04-22.md`
3. `docs/evidence/value_model_migration/UDT_LAYOUT_BOUNDARY_STATUS_2026-04-22.md`
4. `docs/evidence/value_model_migration/POINTER_HELPER_ABI_CELL_RECONCILIATION_2026-04-22.md`
5. `docs/evidence/value_model_migration/NATIVE_DECLARE_WRITEBACK_RECONCILIATION_2026-04-22.md`

## Use

The final migration report should cite this file for grouped artifact selection
and `LATEST_ARTIFACT_MAP.csv` for the flat machine-readable pointer table.
