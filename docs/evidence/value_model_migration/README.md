# Value Model Migration Evidence

This root is the canonical evidence home for the Windows VBA 7.1 x64-style
value-model migration tracked by
[WORKSET_2026-04-20_VALUE_MODEL_MIGRATION_COMPARISON_AND_PERF_PLAN.md](/C:/Work/DnaCalc/OxVba/docs/worksets/WORKSET_2026-04-20_VALUE_MODEL_MIGRATION_COMPARISON_AND_PERF_PLAN.md).

## Layout

- `runs/`
  - immutable timestamped or named harness outputs
  - each run owns `baseline/`, `candidate/`, and `comparison/` subtrees when
    applicable
- `report_inputs/`
  - stable section-oriented placeholders and indexes for the final migration
    report

## Current Harness-Owned Run Families

- correctness: `runs/value_model_correctness_*`
- string perf: `runs/value_model_string_perf_*`
- memory: `runs/value_model_memory_*`

## Stable Entry Points

- report-input index:
  [REPORT_INPUT_INDEX.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/REPORT_INPUT_INDEX.md)
- latest artifact map:
  [LATEST_ARTIFACT_MAP.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/LATEST_ARTIFACT_MAP.csv)

The rule for later beads is simple:

1. new paired run artifacts land under `runs/`
2. `LATEST_ARTIFACT_MAP.csv` is refreshed to point at the currently selected
   canonical inputs
3. report-writing beads consume files from `report_inputs/` rather than
   hardcoding individual run ids in ad hoc locations.
