# Conformance Oracle Run

- Timestamp (UTC): 2026-07-03T01:14:06Z
- Excel version: 16.0
- Excel process id: 4364
- Dialog guardian enabled: True
- Dialog guardian log: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_composite_scalarized_20260703\excel_dialog_guardian.log
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_composite_scalarized_20260703\results.csv

## Summary

| Metric | Count |
|--------|-------|
| Total tests | 15 |
| Oracle OK | 14 |
| Oracle error | 1 |
| Skipped | 0 |
| **Match** | **0** |
| **Mismatch** | **15** |

## Mismatches

| File | Oracle | Golden | Notes |
|------|--------|--------|-------|
| array_explicit_lower_bound.bas | ok: `i16:11` | ok: `empty|i16:11` | oracle=ok/i16:11 golden=ok/empty|i16:11 |
| array_multidim_indexing.bas | ok: `i16:17` | ok: `empty|i16:17` | oracle=ok/i16:17 golden=ok/empty|i16:17 |
| array_option_base_one_bounds.bas | ok: `i16:4|i16:9` | ok: `empty|i16:4|i16:9` | oracle=ok/i16:4|i16:9 golden=ok/empty|i16:4|i16:9 |
| array_store_load.bas | ok: `i16:7` | ok: `empty|i16:7` | oracle=ok/i16:7 golden=ok/empty|i16:7 |
| array_zero_index.bas | ok: `i16:3` | ok: `empty|i16:3` | oracle=ok/i16:3 golden=ok/empty|i16:3 |
| erase_array_basic.bas | ok: `empty` | ok: `empty|empty` | oracle=ok/empty golden=ok/empty|empty |
| for_each_array_variable_basic.bas | ok: `i16:6|empty` | ok: `empty|i16:6|empty` | oracle=ok/i16:6|empty golden=ok/empty|i16:6|empty |
| redim_expand_allows_new_index.bas | ok: `i16:5` | ok: `empty|i16:5` | oracle=ok/i16:5 golden=ok/empty|i16:5 |
| redim_preserve_keeps_values.bas | ok: `i16:7` | ok: `empty|i16:7` | oracle=ok/i16:7 golden=ok/empty|i16:7 |
| redim_preserve_multidim_last_dimension.bas | ok: `i16:7` | ok: `empty|i16:7` | oracle=ok/i16:7 golden=ok/empty|i16:7 |
| redim_preserve_shrink_expand_clears_tail.bas | ok: `empty` | ok: `empty|empty` | oracle=ok/empty golden=ok/empty|empty |
| redim_without_preserve_resets.bas | ok: `empty` | ok: `empty|empty` | oracle=ok/empty golden=ok/empty|empty |
| udt_field_access_basic.bas | ok: `i16:7` | ok: `empty|i32:7|i32:7|i32:7` | oracle=ok/i16:7 golden=ok/empty|i32:7|i32:7|i32:7 |
| udt_whole_assignment_copy.bas | ok: `i16:7|i16:9|i16:7|i16:9|i16:9` | ok: `empty|i32:7|i32:9|empty|i32:7|i32:9|i32:9` | oracle=ok/i16:7|i16:9|i16:7|i16:9|i16:9 golden=ok/empty|i32:7|i32:9|empty|i32:7|i32:9|i32:9 |
| udt_whole_assignment_overwrite.bas | error: `` | ok: `empty|i32:7|i32:6|empty|i32:7|i32:6` | oracle=error/ golden=ok/empty|i32:7|i32:6|empty|i32:7|i32:6; Cannot run the macro 'RunProbe'. The macro may not be available in this workbook or all macros may be disabled. |

## All Results

| File | Oracle Status | Oracle Values | Golden Status | Golden Values | Match |
|------|-------------|-------------|--------------|-------------|-------|
| array_explicit_lower_bound.bas | ok | `i16:11` | ok | `empty|i16:11` | false |
| array_multidim_indexing.bas | ok | `i16:17` | ok | `empty|i16:17` | false |
| array_option_base_one_bounds.bas | ok | `i16:4|i16:9` | ok | `empty|i16:4|i16:9` | false |
| array_store_load.bas | ok | `i16:7` | ok | `empty|i16:7` | false |
| array_zero_index.bas | ok | `i16:3` | ok | `empty|i16:3` | false |
| erase_array_basic.bas | ok | `empty` | ok | `empty|empty` | false |
| for_each_array_variable_basic.bas | ok | `i16:6|empty` | ok | `empty|i16:6|empty` | false |
| redim_expand_allows_new_index.bas | ok | `i16:5` | ok | `empty|i16:5` | false |
| redim_preserve_keeps_values.bas | ok | `i16:7` | ok | `empty|i16:7` | false |
| redim_preserve_multidim_last_dimension.bas | ok | `i16:7` | ok | `empty|i16:7` | false |
| redim_preserve_shrink_expand_clears_tail.bas | ok | `empty` | ok | `empty|empty` | false |
| redim_without_preserve_resets.bas | ok | `empty` | ok | `empty|empty` | false |
| udt_field_access_basic.bas | ok | `i16:7` | ok | `empty|i32:7|i32:7|i32:7` | false |
| udt_whole_assignment_copy.bas | ok | `i16:7|i16:9|i16:7|i16:9|i16:9` | ok | `empty|i32:7|i32:9|empty|i32:7|i32:9|i32:9` | false |
| udt_whole_assignment_overwrite.bas | error | `` | ok | `empty|i32:7|i32:6|empty|i32:7|i32:6` | false |
