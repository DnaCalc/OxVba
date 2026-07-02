# Conformance Oracle Run

- Timestamp (UTC): 2026-07-02T23:18:38Z
- Excel version: 16.0
- Excel process id: 13332
- Dialog guardian enabled: True
- Dialog guardian log: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_basic_scalar_drift_20260702\excel_dialog_guardian.log
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_basic_scalar_drift_20260702\results.csv

## Summary

| Metric | Count |
|--------|-------|
| Total tests | 42 |
| Oracle OK | 41 |
| Oracle error | 1 |
| Skipped | 0 |
| **Match** | **0** |
| **Mismatch** | **42** |

## Mismatches

| File | Oracle | Golden | Notes |
|------|--------|--------|-------|
| call_coercion_mixed_variant_to_long.bas | ok: `i16:5` | ok: `i32:5` | oracle=ok/i16:5 golden=ok/i32:5 |
| class_lifecycle_initialize_fail.bas | ok: `i16:1` | ok: `i32:1` | oracle=ok/i16:1 golden=ok/i32:1 |
| class_lifecycle_resume_next_ok.bas | ok: `i16:3` | ok: `i32:3` | oracle=ok/i16:3 golden=ok/i32:3 |
| class_lifecycle_terminate_fail.bas | ok: `i16:1` | ok: `i32:1` | oracle=ok/i16:1 golden=ok/i32:1 |
| conditional_compilation_basic.bas | ok: `i16:8` | ok: `i32:8` | oracle=ok/i16:8 golden=ok/i32:8 |
| consolidate_error_intrinsics_mix.bas | ok: `i32:12|error:4|bool:true` | ok: `i32:12|error:4|i32:1` | oracle=ok/i32:12|error:4|bool:true golden=ok/i32:12|error:4|i32:1 |
| consolidate_select_conversion.bas | ok: `i16:7|i16:20` | ok: `i32:7|i32:20` | oracle=ok/i16:7|i16:20 golden=ok/i32:7|i32:20 |
| conversion_cint_basic.bas | ok: `i16:5` | ok: `i32:5` | oracle=ok/i16:5 golden=ok/i32:5 |
| conversion_clng_cint_chain.bas | ok: `i16:7|i32:10|i16:8` | ok: `i32:7|i32:10|i32:8` | oracle=ok/i16:7|i32:10|i16:8 golden=ok/i32:7|i32:10|i32:8 |
| conversion_extended_scalar_subset.bas | ok: `f64:7|u8:8|currency:9|decimal:10` | ok: `i32:7|i32:8|i32:9|i32:10` | oracle=ok/f64:7|u8:8|currency:9|decimal:10 golden=ok/i32:7|i32:8|i32:9|i32:10 |
| conversion_val_str_subset.bas | ok: `f64:9` | ok: `i32:9` | oracle=ok/f64:9 golden=ok/i32:9 |
| declaration_collision_proc_name_error.bas | ok: `i16:1` | ok: `i32:1` | oracle=ok/i16:1 golden=ok/i32:1 |
| enum_select_case_use.bas | ok: `i16:20|i32:2` | ok: `i32:3|i32:2|i32:1|i32:20|i32:2` | oracle=ok/i16:20|i32:2 golden=ok/i32:3|i32:2|i32:1|i32:20|i32:2 |
| error_goto_label_resume_next.bas | ok: `i16:20|i32:42` | ok: `i32:20|i32:42` | oracle=ok/i16:20|i32:42 golden=ok/i32:20|i32:42 |
| for_each_array_literal_basic.bas | ok: `i16:3|empty` | ok: `i32:3|empty` | oracle=ok/i16:3|empty golden=ok/i32:3|empty |
| for_exit_for_basic.bas | ok: `i16:1|i16:1` | ok: `i32:1|i32:1` | oracle=ok/i16:1|i16:1 golden=ok/i32:1|i32:1 |
| for_zero_iter.bas | ok: `i16:5|i16:5` | ok: `i32:5|i32:5` | oracle=ok/i16:5|i16:5 golden=ok/i32:5|i32:5 |
| goto_line_number_statement_basic.bas | ok: `i16:5` | ok: `i32:5` | oracle=ok/i16:5 golden=ok/i32:5 |
| if_else_path.bas | ok: `i16:20` | ok: `i32:20` | oracle=ok/i16:20 golden=ok/i32:20 |
| if_elseif_else_path.bas | ok: `i16:99` | ok: `i32:99` | oracle=ok/i16:99 golden=ok/i32:99 |
| if_elseif_path.bas | ok: `i16:30` | ok: `i32:30` | oracle=ok/i16:30 golden=ok/i32:30 |
| if_false.bas | ok: `i16:0` | ok: `i32:0` | oracle=ok/i16:0 golden=ok/i32:0 |
| params_byref_swap.bas | ok: `i16:10|i16:5` | ok: `i32:10|i32:5` | oracle=ok/i16:10|i16:5 golden=ok/i32:10|i32:5 |
| params_byval.bas | ok: `i16:1` | ok: `i32:1` | oracle=ok/i16:1 golden=ok/i32:1 |
| params_named_bind.bas | ok: `i16:9` | ok: `i32:9` | oracle=ok/i16:9 golden=ok/i32:9 |
| params_named_optional_omit.bas | ok: `i16:7` | ok: `i32:7` | oracle=ok/i16:7 golden=ok/i32:7 |
| params_optional_default.bas | ok: `i16:7` | ok: `i32:7` | oracle=ok/i16:7 golden=ok/i32:7 |
| params_optional_override.bas | ok: `i16:9` | ok: `i32:9` | oracle=ok/i16:9 golden=ok/i32:9 |
| property_get_declaration_basic.bas | ok: `i16:4` | ok: `i32:4` | oracle=ok/i16:4 golden=ok/i32:4 |
| property_get_expression_basic.bas | ok: `i16:9` | ok: `i32:9` | oracle=ok/i16:9 golden=ok/i32:9 |
| property_let_byref_route.bas | ok: `i16:1` | ok: `i32:1` | oracle=ok/i16:1 golden=ok/i32:1 |
| regression_cverr_error_resume_bridge.bas | ok: `i32:11|i32:20|empty|empty|bool:false` | ok: `i32:11|i32:20|bool:true|i32:10|bool:false` | oracle=ok/i32:11|i32:20|empty|empty|bool:false golden=ok/i32:11|i32:20|bool:true|i32:10|bool:false |
| resume_next_statement_ok.bas | ok: `i16:1` | ok: `i32:1` | oracle=ok/i16:1 golden=ok/i32:1 |
| select_case_basic.bas | ok: `i16:20` | ok: `i32:20` | oracle=ok/i16:20 golden=ok/i32:20 |
| select_case_else.bas | ok: `i16:99` | ok: `i32:99` | oracle=ok/i16:99 golden=ok/i32:99 |
| select_case_is_range.bas | ok: `i16:22` | ok: `i32:22` | oracle=ok/i16:22 golden=ok/i32:22 |
| select_case_multi.bas | ok: `i16:30` | ok: `i32:30` | oracle=ok/i16:30 golden=ok/i32:30 |
| stdlib_advanced_instrrev_like.bas | ok: `i32:4|i16:1` | ok: `i32:4|i32:1` | oracle=ok/i32:4|i16:1 golden=ok/i32:4|i32:1 |
| stdlib_math_primitives.bas | error: `` | ok: `i32:7|i32:-1|i32:9|i32:20` | oracle=error/ golden=ok/i32:7|i32:-1|i32:9|i32:20; VBA runtime error RTMERR:5 |
| stdlib_numeric_expansion.bas | ok: `string:"1F"|string:"21"|f64:1.460139105621|f64:-225.950846454195` | ok: `string:"1F"|string:"21"|i32:1|i32:-226` | oracle=ok/string:"1F"|string:"21"|f64:1.460139105621|f64:-225.950846454195 golden=ok/string:"1F"|string:"21"|i32:1|i32:-226 |
| stdlib_variant_predicates.bas | ok: `bool:true|bool:true|bool:false` | ok: `i32:1|bool:true|i32:0` | oracle=ok/bool:true|bool:true|bool:false golden=ok/i32:1|bool:true|i32:0 |
| udt_declaration_basic.bas | ok: `i16:9` | ok: `i32:9` | oracle=ok/i16:9 golden=ok/i32:9 |

## All Results

| File | Oracle Status | Oracle Values | Golden Status | Golden Values | Match |
|------|-------------|-------------|--------------|-------------|-------|
| call_coercion_mixed_variant_to_long.bas | ok | `i16:5` | ok | `i32:5` | false |
| class_lifecycle_initialize_fail.bas | ok | `i16:1` | ok | `i32:1` | false |
| class_lifecycle_resume_next_ok.bas | ok | `i16:3` | ok | `i32:3` | false |
| class_lifecycle_terminate_fail.bas | ok | `i16:1` | ok | `i32:1` | false |
| conditional_compilation_basic.bas | ok | `i16:8` | ok | `i32:8` | false |
| consolidate_error_intrinsics_mix.bas | ok | `i32:12|error:4|bool:true` | ok | `i32:12|error:4|i32:1` | false |
| consolidate_select_conversion.bas | ok | `i16:7|i16:20` | ok | `i32:7|i32:20` | false |
| conversion_cint_basic.bas | ok | `i16:5` | ok | `i32:5` | false |
| conversion_clng_cint_chain.bas | ok | `i16:7|i32:10|i16:8` | ok | `i32:7|i32:10|i32:8` | false |
| conversion_extended_scalar_subset.bas | ok | `f64:7|u8:8|currency:9|decimal:10` | ok | `i32:7|i32:8|i32:9|i32:10` | false |
| conversion_val_str_subset.bas | ok | `f64:9` | ok | `i32:9` | false |
| declaration_collision_proc_name_error.bas | ok | `i16:1` | ok | `i32:1` | false |
| enum_select_case_use.bas | ok | `i16:20|i32:2` | ok | `i32:3|i32:2|i32:1|i32:20|i32:2` | false |
| error_goto_label_resume_next.bas | ok | `i16:20|i32:42` | ok | `i32:20|i32:42` | false |
| for_each_array_literal_basic.bas | ok | `i16:3|empty` | ok | `i32:3|empty` | false |
| for_exit_for_basic.bas | ok | `i16:1|i16:1` | ok | `i32:1|i32:1` | false |
| for_zero_iter.bas | ok | `i16:5|i16:5` | ok | `i32:5|i32:5` | false |
| goto_line_number_statement_basic.bas | ok | `i16:5` | ok | `i32:5` | false |
| if_else_path.bas | ok | `i16:20` | ok | `i32:20` | false |
| if_elseif_else_path.bas | ok | `i16:99` | ok | `i32:99` | false |
| if_elseif_path.bas | ok | `i16:30` | ok | `i32:30` | false |
| if_false.bas | ok | `i16:0` | ok | `i32:0` | false |
| params_byref_swap.bas | ok | `i16:10|i16:5` | ok | `i32:10|i32:5` | false |
| params_byval.bas | ok | `i16:1` | ok | `i32:1` | false |
| params_named_bind.bas | ok | `i16:9` | ok | `i32:9` | false |
| params_named_optional_omit.bas | ok | `i16:7` | ok | `i32:7` | false |
| params_optional_default.bas | ok | `i16:7` | ok | `i32:7` | false |
| params_optional_override.bas | ok | `i16:9` | ok | `i32:9` | false |
| property_get_declaration_basic.bas | ok | `i16:4` | ok | `i32:4` | false |
| property_get_expression_basic.bas | ok | `i16:9` | ok | `i32:9` | false |
| property_let_byref_route.bas | ok | `i16:1` | ok | `i32:1` | false |
| regression_cverr_error_resume_bridge.bas | ok | `i32:11|i32:20|empty|empty|bool:false` | ok | `i32:11|i32:20|bool:true|i32:10|bool:false` | false |
| resume_next_statement_ok.bas | ok | `i16:1` | ok | `i32:1` | false |
| select_case_basic.bas | ok | `i16:20` | ok | `i32:20` | false |
| select_case_else.bas | ok | `i16:99` | ok | `i32:99` | false |
| select_case_is_range.bas | ok | `i16:22` | ok | `i32:22` | false |
| select_case_multi.bas | ok | `i16:30` | ok | `i32:30` | false |
| stdlib_advanced_instrrev_like.bas | ok | `i32:4|i16:1` | ok | `i32:4|i32:1` | false |
| stdlib_math_primitives.bas | error | `` | ok | `i32:7|i32:-1|i32:9|i32:20` | false |
| stdlib_numeric_expansion.bas | ok | `string:"1F"|string:"21"|f64:1.460139105621|f64:-225.950846454195` | ok | `string:"1F"|string:"21"|i32:1|i32:-226` | false |
| stdlib_variant_predicates.bas | ok | `bool:true|bool:true|bool:false` | ok | `i32:1|bool:true|i32:0` | false |
| udt_declaration_basic.bas | ok | `i16:9` | ok | `i32:9` | false |
