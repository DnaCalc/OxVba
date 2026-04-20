# Fixed-Tag Baseline Old Artifact Index

This file is the canonical index for the fixed baseline tag
`pre-value-model-migration-2026-04-20` (`old`) artifacts selected for the
value-model migration comparison.

These artifacts were produced by the migration-owned paired harnesses. The
baseline side of each run is the old implementation reference that later
migration beads compare against.

## Selected Runs

| Family | Selected run | Baseline-side entry point |
|---|---|---|
| correctness | `vmc4-smoke-focused-exact` | [baseline/correctness](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/baseline/correctness) |
| string perf | `vmsp1-smoke-vm-small` | [string_perf.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_string_perf_vmsp1-smoke-vm-small/baseline/perf/string_perf.csv) |
| memory | `vmmem3-smoke` | [baseline/memory](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_memory_vmmem3-smoke/baseline/memory) |

## Correctness

- selected run: `vmc4-smoke-focused-exact`
- paired summary:
  [correctness_summary.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/correctness_summary.csv)
- paired comparison:
  [correctness_summary.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/comparison/correctness_summary.md)
- fixed-tag baseline lane logs:
  - [dispatch_exception_details.log.txt](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/baseline/correctness/dispatch_exception_details.log.txt)
  - [dispatch_exception_resume_next.log.txt](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/baseline/correctness/dispatch_exception_resume_next.log.txt)
  - [dispatch_exception_rich_excepinfo.log.txt](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/baseline/correctness/dispatch_exception_rich_excepinfo.log.txt)
  - [event_callback_handler_body.log.txt](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/baseline/correctness/event_callback_handler_body.log.txt)
  - [event_callback_value_payload.log.txt](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/baseline/correctness/event_callback_value_payload.log.txt)
  - [registered_event_callback_identity.log.txt](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/baseline/correctness/registered_event_callback_identity.log.txt)
  - [pointer_variant_scalar_container.log.txt](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/baseline/correctness/pointer_variant_scalar_container.log.txt)
  - [pointer_variant_decimal_container.log.txt](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/baseline/correctness/pointer_variant_decimal_container.log.txt)
  - [pointer_variant_object_rejected.log.txt](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/baseline/correctness/pointer_variant_object_rejected.log.txt)
  - [pointer_variant_array_rejected.log.txt](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/baseline/correctness/pointer_variant_array_rejected.log.txt)
  - [native_string_writeback_array_slot.log.txt](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_correctness_vmc4-smoke-focused-exact/baseline/correctness/native_string_writeback_array_slot.log.txt)

## String Perf

- selected run: `vmsp1-smoke-vm-small`
- paired summary:
  [string_perf_summary.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_string_perf_vmsp1-smoke-vm-small/string_perf_summary.csv)
- paired comparison:
  [string_perf_summary.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_string_perf_vmsp1-smoke-vm-small/comparison/string_perf_summary.md)
- fixed-tag baseline string perf:
  [baseline/perf/string_perf.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_string_perf_vmsp1-smoke-vm-small/baseline/perf/string_perf.csv)

## Memory

- selected run: `vmmem3-smoke`
- paired summaries:
  - [layout_metrics_summary.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_memory_vmmem3-smoke/layout_metrics_summary.csv)
  - [process_memory_summary.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_memory_vmmem3-smoke/process_memory_summary.csv)
  - [pointer_snapshot_summary.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_memory_vmmem3-smoke/pointer_snapshot_summary.csv)
- fixed-tag baseline memory artifacts:
  - [baseline/memory/layout_metrics.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_memory_vmmem3-smoke/baseline/memory/layout_metrics.csv)
  - [baseline/memory/process_memory.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_memory_vmmem3-smoke/baseline/memory/process_memory.csv)
  - [baseline/memory/pointer_snapshot_summary.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/runs/value_model_memory_vmmem3-smoke/baseline/memory/pointer_snapshot_summary.csv)
