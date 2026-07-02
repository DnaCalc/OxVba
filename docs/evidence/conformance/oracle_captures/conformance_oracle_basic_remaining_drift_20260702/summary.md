# Conformance Oracle Run

- Timestamp (UTC): 2026-07-02T23:26:54Z
- Excel version: 16.0
- Excel process id: 35464
- Dialog guardian enabled: True
- Dialog guardian log: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_basic_remaining_drift_20260702\excel_dialog_guardian.log
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_basic_remaining_drift_20260702\results.csv

## Summary

| Metric | Count |
|--------|-------|
| Total tests | 31 |
| Oracle OK | 20 |
| Oracle error | 8 |
| Skipped | 3 |
| **Match** | **6** |
| **Mismatch** | **22** |

## Mismatches

| File | Oracle | Golden | Notes |
|------|--------|--------|-------|
| array_explicit_lower_bound.bas | ok: `empty|i16:11` | ok: `empty|empty|i32:11|empty|i32:11` | oracle=ok/empty|i16:11 golden=ok/empty|empty|i32:11|empty|i32:11 |
| array_multidim_indexing.bas | ok: `empty|i16:17` | ok: `empty|empty|empty|empty|empty|empty|i32:17|i32:17` | oracle=ok/empty|i16:17 golden=ok/empty|empty|empty|empty|empty|empty|i32:17|i32:17 |
| array_option_base_one_bounds.bas | ok: `empty|i16:4|i16:9` | ok: `empty|i32:4|empty|i32:9|i32:4|i32:9` | oracle=ok/empty|i16:4|i16:9 golden=ok/empty|i32:4|empty|i32:9|i32:4|i32:9 |
| array_store_load.bas | ok: `empty|i16:7` | ok: `empty|i32:5|i32:7|empty|i32:7` | oracle=ok/empty|i16:7 golden=ok/empty|i32:5|i32:7|empty|i32:7 |
| array_zero_index.bas | ok: `empty|i16:3` | ok: `empty|i32:3|empty|empty|i32:3` | oracle=ok/empty|i16:3 golden=ok/empty|i32:3|empty|empty|i32:3 |
| coercion_cverr_abs_normalization.bas | error: `` | ok: `error:4|error:4|bool:true` | oracle=error/ golden=ok/error:4|error:4|bool:true; VBA runtime error RTMERR:5 |
| enum_arithmetic_use.bas | ok: `i32:6|i32:9` | ok: `i32:10|i32:1|i32:5|i32:6|i32:9` | oracle=ok/i32:6|i32:9 golden=ok/i32:10|i32:1|i32:5|i32:6|i32:9 |
| enum_basic.bas | ok: `i32:5` | ok: `i32:3|i32:4|i32:5` | oracle=ok/i32:5 golden=ok/i32:3|i32:4|i32:5 |
| erase_array_basic.bas | ok: `empty|empty` | ok: `empty|empty|empty|empty|empty` | oracle=ok/empty|empty golden=ok/empty|empty|empty|empty|empty |
| err_surface_fields_subset.bas | ok: `i32:9|string:"Subscript out of range"|string:"VBAProject"|i32:1000009|string:"C:\Program Files\Common Files\Microsoft Shared\VBA\VBA7.1\1033\VbLR6.chm"|i32:0` | ok: `i32:9|string:""|string:""|i32:0|string:""|i32:0` | oracle=ok/i32:9|string:"Subscript out of range"|string:"VBAProject"|i32:1000009|string:"C:\Program Files\Common Files\Microsoft Shared\VBA\VBA7.1\1033\VbLR6.chm"|i32:0 golden=ok/i32:9|string:""|string:""|i32:0|string:""|i32:0 |
| for_each_array_variable_basic.bas | ok: `empty|i16:6|empty` | ok: `empty|i32:4|i32:5|i32:6|i32:6|i32:6` | oracle=ok/empty|i16:6|empty golden=ok/empty|i32:4|i32:5|i32:6|i32:6|i32:6 |
| module_const_basic.bas | ok: `i16:7` | ok: `i32:5|i32:7` | oracle=ok/i16:7 golden=ok/i32:5|i32:7 |
| property_set_byref_route.bas | error: `` | ok: `i32:2` | oracle=error/ golden=ok/i32:2; 0x800A9C68 |
| redim_expand_allows_new_index.bas | ok: `empty|i16:5` | ok: `empty|empty|empty|i32:5|empty|i32:5` | oracle=ok/empty|i16:5 golden=ok/empty|empty|empty|i32:5|empty|i32:5 |
| redim_preserve_keeps_values.bas | ok: `empty|i16:7` | ok: `empty|i32:7|empty|i32:7|empty|empty` | oracle=ok/empty|i16:7 golden=ok/empty|i32:7|empty|i32:7|empty|empty |
| redim_preserve_multidim_last_dimension.bas | ok: `empty|i16:7` | ok: `empty|i32:7|empty|empty|empty|i32:7|empty|empty` | oracle=ok/empty|i16:7 golden=ok/empty|i32:7|empty|empty|empty|i32:7|empty|empty |
| redim_preserve_shrink_expand_clears_tail.bas | ok: `empty|empty` | ok: `empty|empty|empty|empty|empty|empty` | oracle=ok/empty|empty golden=ok/empty|empty|empty|empty|empty|empty |
| redim_without_preserve_resets.bas | ok: `empty|empty` | ok: `empty|empty|empty|empty|empty|empty` | oracle=ok/empty|empty golden=ok/empty|empty|empty|empty|empty|empty |
| udt_field_access_basic.bas | error: `` | ok: `empty|i32:7|i32:7|i32:7` | oracle=error/ golden=ok/empty|i32:7|i32:7|i32:7; 0x800A9C68 |
| udt_whole_assignment_copy.bas | error: `` | ok: `empty|i32:7|i32:9|empty|i32:7|i32:9|i32:9` | oracle=error/ golden=ok/empty|i32:7|i32:9|empty|i32:7|i32:9|i32:9; 0x800A9C68 |
| udt_whole_assignment_overwrite.bas | error: `` | ok: `empty|i32:7|i32:6|empty|i32:7|i32:6` | oracle=error/ golden=ok/empty|i32:7|i32:6|empty|i32:7|i32:6; Cannot run the macro 'RunProbe'. The macro may not be available in this workbook or all macros may be disabled. |
| with_block_basic.bas | error: `` | ok: `i32:3|i32:3` | oracle=error/ golden=ok/i32:3|i32:3; VBA runtime error RTMERR:424 |

## All Results

| File | Oracle Status | Oracle Values | Golden Status | Golden Values | Match |
|------|-------------|-------------|--------------|-------------|-------|
| array_explicit_lower_bound.bas | ok | `empty|i16:11` | ok | `empty|empty|i32:11|empty|i32:11` | false |
| array_multidim_indexing.bas | ok | `empty|i16:17` | ok | `empty|empty|empty|empty|empty|empty|i32:17|i32:17` | false |
| array_option_base_one_bounds.bas | ok | `empty|i16:4|i16:9` | ok | `empty|i32:4|empty|i32:9|i32:4|i32:9` | false |
| array_store_load.bas | ok | `empty|i16:7` | ok | `empty|i32:5|i32:7|empty|i32:7` | false |
| array_zero_index.bas | ok | `empty|i16:3` | ok | `empty|i32:3|empty|empty|i32:3` | false |
| coercion_cverr_abs_normalization.bas | error | `` | ok | `error:4|error:4|bool:true` | false |
| default_type_param_defobj_error.bas | error | `` | error | `` | true |
| enum_arithmetic_use.bas | ok | `i32:6|i32:9` | ok | `i32:10|i32:1|i32:5|i32:6|i32:9` | false |
| enum_basic.bas | ok | `i32:5` | ok | `i32:3|i32:4|i32:5` | false |
| erase_array_basic.bas | ok | `empty|empty` | ok | `empty|empty|empty|empty|empty` | false |
| err_surface_fields_subset.bas | ok | `i32:9|string:"Subscript out of range"|string:"VBAProject"|i32:1000009|string:"C:\Program Files\Common Files\Microsoft Shared\VBA\VBA7.1\1033\VbLR6.chm"|i32:0` | ok | `i32:9|string:""|string:""|i32:0|string:""|i32:0` | false |
| error_nested_mode_transitions.bas | ok | `i32:5|i32:20|i32:0|i32:6` | ok | `i32:5|i32:20|i32:0|i32:6` | true |
| for_each_array_literal_basic.bas | ok | `i16:3|empty` | ok | `i16:3|empty` | true |
| for_each_array_variable_basic.bas | ok | `empty|i16:6|empty` | ok | `empty|i32:4|i32:5|i32:6|i32:6|i32:6` | false |
| for_exit_for_basic.bas | ok | `i16:1|i16:1` | ok | `i16:1|i16:1` | true |
| function_return_explicit_as_precedence_error.bas | error | `` | error | `` | true |
| module_const_basic.bas | ok | `i16:7` | ok | `i32:5|i32:7` | false |
| object_collection_add_item.bas | skip | `` | ok | `i32:1|i32:1` |  |
| object_collection_count_chain.bas | skip | `` | ok | `i32:2|i32:2` |  |
| object_collection_remove_count.bas | skip | `` | ok | `i32:0` |  |
| property_set_byref_route.bas | error | `` | ok | `i32:2` | false |
| redim_expand_allows_new_index.bas | ok | `empty|i16:5` | ok | `empty|empty|empty|i32:5|empty|i32:5` | false |
| redim_preserve_keeps_values.bas | ok | `empty|i16:7` | ok | `empty|i32:7|empty|i32:7|empty|empty` | false |
| redim_preserve_multidim_last_dimension.bas | ok | `empty|i16:7` | ok | `empty|i32:7|empty|empty|empty|i32:7|empty|empty` | false |
| redim_preserve_shrink_expand_clears_tail.bas | ok | `empty|empty` | ok | `empty|empty|empty|empty|empty|empty` | false |
| redim_without_preserve_resets.bas | ok | `empty|empty` | ok | `empty|empty|empty|empty|empty|empty` | false |
| regression_cverr_error_resume_bridge.bas | ok | `i32:11|i32:20|empty|empty|bool:false` | ok | `i32:11|i32:20|empty|empty|bool:false` | true |
| udt_field_access_basic.bas | error | `` | ok | `empty|i32:7|i32:7|i32:7` | false |
| udt_whole_assignment_copy.bas | error | `` | ok | `empty|i32:7|i32:9|empty|i32:7|i32:9|i32:9` | false |
| udt_whole_assignment_overwrite.bas | error | `` | ok | `empty|i32:7|i32:6|empty|i32:7|i32:6` | false |
| with_block_basic.bas | error | `` | ok | `i32:3|i32:3` | false |
