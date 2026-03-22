# Conformance Oracle Run

- Timestamp (UTC): 2026-03-21T09:50:03Z
- Excel version: 16.0
- Excel process id: 67052
- Dialog guardian enabled: True
- Dialog guardian log: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_20260321T094401Z\excel_dialog_guardian.log
- Output CSV: C:\Work\DnaCalc\OxVba\docs\evidence\conformance\oracle_captures\conformance_oracle_20260321T094401Z\results.csv

## Summary

| Metric | Count |
|--------|-------|
| Total tests | 192 |
| Oracle OK | 114 |
| Oracle error | 63 |
| Skipped | 15 |
| **Match** | **116** |
| **Mismatch** | **61** |

## Mismatches

| File | Oracle | Golden | Notes |
|------|--------|--------|-------|
| array_explicit_lower_bound.bas | ok: `0,11` | ok: `0,11,0,11` | oracle=ok/0,11 golden=ok/0,11,0,11 |
| array_multidim_indexing.bas | ok: `0,17` | ok: `0,0,0,0,0,17,17` | oracle=ok/0,17 golden=ok/0,0,0,0,0,17,17 |
| array_option_base_one_bounds.bas | ok: `0,4,9` | ok: `4,0,9,4,9` | oracle=ok/0,4,9 golden=ok/4,0,9,4,9 |
| array_store_load.bas | ok: `0,7` | ok: `5,7,0,7` | oracle=ok/0,7 golden=ok/5,7,0,7 |
| array_zero_index.bas | ok: `0,3` | ok: `3,0,0,3` | oracle=ok/0,3 golden=ok/3,0,0,3 |
| assignment_set_let_basic.bas | error: `` | ok: `7` | oracle=error/ golden=ok/7; 0x800A9C68 |
| class_lifecycle_initialize_fail.bas | ok: `1` | error: `` | oracle=ok/1 golden=error/ |
| class_lifecycle_terminate_fail.bas | ok: `1` | error: `` | oracle=ok/1 golden=error/ |
| coercion_cverr_abs_normalization.bas | error: `` | ok: `-899999996,-899999996,1` | oracle=error/ golden=ok/-899999996,-899999996,1; VBA runtime error RTMERR:5 |
| coercion_cverr_range_predicates.bas | error: `` | ok: `1,1,1,1,0,10` | oracle=error/ golden=ok/1,1,1,1,0,10; VBA runtime error RTMERR:5 |
| declaration_collision_proc_name_error.bas | ok: `1` | error: `` | oracle=ok/1 golden=error/ |
| enum_basic.bas | ok: `5` | ok: `3,4,5` | oracle=ok/5 golden=ok/3,4,5 |
| erase_array_basic.bas | ok: `0,0` | ok: `0,0,0,0,0` | oracle=ok/0,0 golden=ok/0,0,0,0,0 |
| err_proc_call_boundary_clears.bas | ok: `7` | ok: `0` | oracle=ok/7 golden=ok/0 |
| err_resume_next_clears.bas | ok: `20` | ok: `0` | oracle=ok/20 golden=ok/0 |
| err_surface_fields_subset.bas | ok: `9,0,0,1000009,0,0` | ok: `9,9,0,0,0,0` | oracle=ok/9,0,0,1000009,0,0 golden=ok/9,9,0,0,0,0 |
| error_nested_mode_transitions.bas | ok: `5,20,0,6` | ok: `5,0,0,6` | oracle=ok/5,20,0,6 golden=ok/5,0,0,6 |
| financial_algorithm_npv_irr_mirr_subset.bas | error: `` | ok: `59,-50,-28` | oracle=error/ golden=ok/59,-50,-28; The remote procedure call failed. (0x800706BE) |
| financial_algorithm_rate_nper_subset.bas | error: `` | ok: `-99,-38` | oracle=error/ golden=ok/-99,-38; VBA runtime error RTMERR:5 |
| financial_tolerance_mixed_modes.bas | error: `` | ok: `-899997999,-899997998,-99,-38` | oracle=error/ golden=ok/-899997999,-899997998,-99,-38; VBA runtime error RTMERR:5 |
| financial_tolerance_non_convergence.bas | error: `` | ok: `-899997999,-899997998` | oracle=error/ golden=ok/-899997999,-899997998; VBA runtime error RTMERR:5 |
| for_each_array_literal_basic.bas | ok: `3,0` | ok: `3,3` | oracle=ok/3,0 golden=ok/3,3 |
| for_each_array_variable_basic.bas | ok: `0,6,0` | ok: `4,5,6,6,6` | oracle=ok/0,6,0 golden=ok/4,5,6,6,6 |
| introspection_vartype_isnumeric_tags.bas | ok: `8,1,10,2,0,0,0,1` | ok: `0,1,10,3,0,0,0,1` | oracle=ok/8,1,10,2,0,0,0,1 golden=ok/0,1,10,3,0,0,0,1 |
| late_bound_default_member_exec.bas | error: `` | ok: `0,7` | oracle=error/ golden=ok/0,7; VBA runtime error RTMERR:91 |
| late_bound_named_argument_exec.bas | error: `` | ok: `0,1` | oracle=error/ golden=ok/0,1; 0x800A9C68 |
| module_const_basic.bas | ok: `7` | ok: `5,7` | oracle=ok/7 golden=ok/5,7 |
| on_error_goto_label_resume.bas | ok: `100` | ok: `6` | oracle=ok/100 golden=ok/6 |
| params_byref_error.bas | ok: `` | error: `` | oracle=ok/ golden=error/ |
| property_get_expression_basic.bas | ok: `9` | ok: `0` | oracle=ok/9 golden=ok/0 |
| property_let_byref_route.bas | ok: `1` | ok: `3` | oracle=ok/1 golden=ok/3 |
| property_set_byref_route.bas | error: `` | ok: `7` | oracle=error/ golden=ok/7; 0x800A9C68 |
| redim_expand_allows_new_index.bas | error: `` | ok: `0,0,5,0,5` | oracle=error/ golden=ok/0,0,5,0,5; 0x800A9C68 |
| redim_preserve_keeps_values.bas | error: `` | ok: `7,0,7,0,0` | oracle=error/ golden=ok/7,0,7,0,0; 0x800A9C68 |
| redim_preserve_multidim_last_dimension.bas | error: `` | ok: `7,0,0,0,7,0,0` | oracle=error/ golden=ok/7,0,0,0,7,0,0; 0x800A9C68 |
| redim_preserve_shrink_expand_clears_tail.bas | error: `` | ok: `0,0,0,0,0` | oracle=error/ golden=ok/0,0,0,0,0; 0x800A9C68 |
| redim_without_preserve_resets.bas | error: `` | ok: `0,0,0,0,0` | oracle=error/ golden=ok/0,0,0,0,0; 0x800A9C68 |
| regression_cverr_error_resume_bridge.bas | ok: `11,20,0,0,0` | ok: `11,0,1,10,0` | oracle=ok/11,20,0,0,0 golden=ok/11,0,1,10,0 |
| stdlib_advanced_split_join.bas | error: `` | ok: `3,789` | oracle=error/ golden=ok/3,789; VBA runtime error RTMERR:13 |
| stdlib_array_introspection_bounds.bas | ok: `0,0,2` | ok: `-999999997,0,2` | oracle=ok/0,0,2 golden=ok/-999999997,0,2 |
| stdlib_array_introspection_types.bas | ok: `0,8204,0` | ok: `-999999998,8204,1001` | oracle=ok/0,8204,0 golden=ok/-999999998,8204,1001 |
| stdlib_date_add_diff.bas | error: `` | ok: `20260231,3` | oracle=error/ golden=ok/20260231,3; VBA runtime error RTMERR:5 |
| stdlib_date_serial_value.bas | ok: `46081,46081` | ok: `20260228,20260228` | oracle=ok/46081,46081 golden=ok/20260228,20260228 |
| stdlib_datetime_expansion.bas | error: `` | ok: `20260301,123456,20260301,20240203,20240203,20240203,20240203,2,42` | oracle=error/ golden=ok/20260301,123456,20260301,20240203,20240203,20240203,20240203,2,42; VBA runtime error RTMERR:13 |
| stdlib_file_stub_intrinsics.bas | error: `` | ok: `1,3,4,5` | oracle=error/ golden=ok/1,3,4,5; VBA runtime error RTMERR:52 |
| stdlib_format_core.bas | ok: `12345,1` | ok: `12345,678` | oracle=ok/12345,1 golden=ok/12345,678 |
| stdlib_introspection_expansion.bas | error: `` | ok: `1,1,1,1` | oracle=error/ golden=ok/1,1,1,1; 0x800A9C68 |
| stdlib_len_basic.bas | error: `` | ok: `4` | oracle=error/ golden=ok/4; 0x800A9C68 |
| stdlib_math_primitives.bas | error: `` | ok: `7,-1,9,20` | oracle=error/ golden=ok/7,-1,9,20; VBA runtime error RTMERR:5 |
| stdlib_numeric_expansion.bas | ok: `0,21,1,-226` | ok: `31,17,9,11` | oracle=ok/0,21,1,-226 golden=ok/31,17,9,11 |
| stdlib_random_financial_expansion.bas | error: `` | ok: `1,42,59,-50,-28,-99,-38` | oracle=error/ golden=ok/1,42,59,-50,-28,-99,-38; The remote procedure call failed. (0x800706BE) |
| stdlib_string_expansion_core.bas | ok: `0,0,0,66,777` | ok: `4,3,66,66,777` | oracle=ok/0,0,0,66,777 golden=ok/4,3,66,66,777 |
| stdlib_time_serial_value.bas | ok: `0,0` | ok: `3723,3723` | oracle=ok/0,0 golden=ok/3723,3723 |
| string_join_array_tag_count.bas | ok: `0,10203` | ok: `-999999997,3` | oracle=ok/0,10203 golden=ok/-999999997,3 |
| string_vbnullstring_predicates.bas | ok: `0,0,0,0` | ok: `0,1,0,0` | oracle=ok/0,0,0,0 golden=ok/0,1,0,0 |
| typeof_is_condition_basic.bas | error: `` | ok: `1` | oracle=error/ golden=ok/1; 0x800A9C68 |
| udt_field_access_basic.bas | error: `` | ok: `0,7,7,7` | oracle=error/ golden=ok/0,7,7,7; 0x800A9C68 |
| udt_whole_assignment_copy.bas | error: `` | ok: `0,7,9,0,7,9,9` | oracle=error/ golden=ok/0,7,9,0,7,9,9; 0x800A9C68 |
| udt_whole_assignment_overwrite.bas | error: `` | ok: `0,7,6,0,7,6` | oracle=error/ golden=ok/0,7,6,0,7,6; Cannot run the macro 'RunProbe'. The macro may not be available in this workbook or all macros may be disabled. |
| with_block_basic.bas | error: `` | ok: `3,3` | oracle=error/ golden=ok/3,3; VBA runtime error RTMERR:424 |
| with_block_member_target_chain.bas | error: `` | ok: `7,7` | oracle=error/ golden=ok/7,7; VBA runtime error RTMERR:424 |

## All Results

| File | Oracle Status | Oracle Slots | Golden Status | Golden Slots | Match |
|------|-------------|-------------|--------------|-------------|-------|
| array_bounds_error.bas | error | `` | error | `` | true |
| array_explicit_lower_bound.bas | ok | `0,11` | ok | `0,11,0,11` | false |
| array_multidim_indexing.bas | ok | `0,17` | ok | `0,0,0,0,0,17,17` | false |
| array_option_base_one_bounds.bas | ok | `0,4,9` | ok | `4,0,9,4,9` | false |
| array_store_load.bas | ok | `0,7` | ok | `5,7,0,7` | false |
| array_zero_index.bas | ok | `0,3` | ok | `3,0,0,3` | false |
| assignment_set_let_basic.bas | error | `` | ok | `7` | false |
| byref_typed_mismatch_error.bas | error | `` | error | `` | true |
| call_coercion_mixed_variant_to_long.bas | ok | `5` | ok | `5` | true |
| class_lifecycle_initialize_fail.bas | ok | `1` | error | `` | false |
| class_lifecycle_resume_next_ok.bas | ok | `3` | ok | `3` | true |
| class_lifecycle_terminate_fail.bas | ok | `1` | error | `` | false |
| coercion_arg_object_to_long_error.bas | error | `` | error | `` | true |
| coercion_assign_object_to_long_error.bas | error | `` | error | `` | true |
| coercion_cverr_abs_normalization.bas | error | `` | ok | `-899999996,-899999996,1` | false |
| coercion_cverr_range_predicates.bas | error | `` | ok | `1,1,1,1,0,10` | false |
| coercion_null_empty_error_predicates.bas | ok | `1,1,1,0,0` | ok | `1,1,1,0,0` | true |
| com_dispatch_array_argument.bas | skip | `` | ok | `25013` |  |
| com_dispatch_createobject_invoke.bas | skip | `` | ok | `5016` |  |
| com_dispatch_invoke_chain.bas | skip | `` | ok | `5009,5014` |  |
| com_dispatch_invoke_deterministic.bas | skip | `` | ok | `5018` |  |
| conditional_compilation_basic.bas | ok | `8` | ok | `8` | true |
| consolidate_collection_host_mix.bas | skip | `` | ok | `1,1,1` |  |
| consolidate_error_intrinsics_mix.bas | ok | `12,-899999996,1` | ok | `12,-899999996,1` | true |
| consolidate_language_stdlib_mix.bas | skip | `` | ok | `3,5008` |  |
| conversion_cint_basic.bas | ok | `5` | ok | `5` | true |
| conversion_cint_to_object_error.bas | error | `` | error | `` | true |
| conversion_extended_scalar_subset.bas | ok | `7,8,9,10` | ok | `7,8,9,10` | true |
| conversion_nested_clng_cint.bas | ok | `7` | ok | `7` | true |
| conversion_val_str_subset.bas | ok | `9` | ok | `9` | true |
| declaration_collision_proc_name_error.bas | ok | `1` | error | `` | false |
| declare_function_stub_basic.bas | skip | `` | error | `` |  |
| declare_sub_stub_basic.bas | skip | `` | error | `` |  |
| default_type_defobj_implicit_error.bas | error | `` | error | `` | true |
| default_type_param_defobj_error.bas | error | `` | error | `` | true |
| diagnostic_phase_compile_wins.bas | error | `` | error | `` | true |
| do_exit_do.bas | ok | `4` | ok | `4` | true |
| do_loop_until_basic.bas | ok | `3` | ok | `3` | true |
| do_loop_while_basic.bas | ok | `3` | ok | `3` | true |
| do_until_basic.bas | ok | `3` | ok | `3` | true |
| do_while_basic.bas | ok | `3` | ok | `3` | true |
| duplicate_dim_error.bas | error | `` | error | `` | true |
| duplicate_label_error.bas | error | `` | error | `` | true |
| enum_basic.bas | ok | `5` | ok | `3,4,5` | false |
| erase_array_basic.bas | ok | `0,0` | ok | `0,0,0,0,0` | false |
| err_clear_basic.bas | ok | `0` | ok | `0` | true |
| err_clear_full_surface_reset.bas | ok | `0,0,0,0,0,0` | ok | `0,0,0,0,0,0` | true |
| err_proc_call_boundary_clears.bas | ok | `7` | ok | `0` | false |
| err_resume_next_clears.bas | ok | `20` | ok | `0` | false |
| err_surface_fields_subset.bas | ok | `9,0,0,1000009,0,0` | ok | `9,9,0,0,0,0` | false |
| error_nested_mode_transitions.bas | ok | `5,20,0,6` | ok | `5,0,0,6` | false |
| financial_algorithm_npv_irr_mirr_subset.bas | error | `` | ok | `59,-50,-28` | false |
| financial_algorithm_rate_nper_subset.bas | error | `` | ok | `-99,-38` | false |
| financial_tolerance_mixed_modes.bas | error | `` | ok | `-899997999,-899997998,-99,-38` | false |
| financial_tolerance_non_convergence.bas | error | `` | ok | `-899997999,-899997998` | false |
| for_basic.bas | ok | `6,4` | ok | `6,4` | true |
| for_each_array_literal_basic.bas | ok | `3,0` | ok | `3,3` | false |
| for_each_array_variable_basic.bas | ok | `0,6,0` | ok | `4,5,6,6,6` | false |
| for_exit_for_basic.bas | ok | `1,1` | ok | `1,1` | true |
| for_step_negative.bas | ok | `3,-1` | ok | `3,-1` | true |
| for_step_positive.bas | ok | `6,7` | ok | `6,7` | true |
| for_step_zero_error.bas | error | `` | error | `` | true |
| for_zero_iter.bas | ok | `5,5` | ok | `5,5` | true |
| function_call_basic.bas | ok | `9` | ok | `9` | true |
| function_return_explicit_as_precedence_error.bas | error | `` | error | `` | true |
| gosub_basic.bas | ok | `4` | ok | `4` | true |
| gosub_missing_label_error.bas | error | `` | error | `` | true |
| gosub_repeated.bas | ok | `5` | ok | `5` | true |
| goto_label_basic.bas | ok | `3` | ok | `3` | true |
| goto_line_number_statement_basic.bas | ok | `5` | ok | `5` | true |
| goto_missing_label_error.bas | error | `` | error | `` | true |
| goto_numeric_basic.bas | ok | `5` | ok | `5` | true |
| if_and.bas | ok | `7` | ok | `7` | true |
| if_else_path.bas | ok | `20` | ok | `20` | true |
| if_elseif_else_path.bas | ok | `99` | ok | `99` | true |
| if_elseif_path.bas | ok | `30` | ok | `30` | true |
| if_false.bas | ok | `0` | ok | `0` | true |
| if_ge.bas | ok | `5` | ok | `5` | true |
| if_lt.bas | ok | `4` | ok | `4` | true |
| if_neq.bas | ok | `11` | ok | `11` | true |
| if_or_not.bas | ok | `9` | ok | `9` | true |
| if_true.bas | ok | `7` | ok | `7` | true |
| introspection_vartype_isnumeric_tags.bas | ok | `8,1,10,2,0,0,0,1` | ok | `0,1,10,3,0,0,0,1` | false |
| jit_intrinsic_math_subset.bas | ok | `1` | ok | `1` | true |
| late_bound_default_member_error.bas | error | `` | error | `` | true |
| late_bound_default_member_exec.bas | error | `` | ok | `0,7` | false |
| late_bound_named_argument_exec.bas | error | `` | ok | `0,1` | false |
| late_call_named_argument_error.bas | error | `` | error | `` | true |
| line_continuation_basic.bas | ok | `3` | ok | `3` | true |
| module_const_basic.bas | ok | `7` | ok | `5,7` | false |
| nested_if_for.bas | ok | `5,3` | ok | `5,3` | true |
| object_collection_add_item.bas | skip | `` | ok | `1,1` |  |
| object_collection_count_chain.bas | skip | `` | ok | `2,2` |  |
| object_collection_remove_count.bas | skip | `` | ok | `0` |  |
| on_error_default_fail.bas | error | `` | error | `` | true |
| on_error_goto_label_missing_label_error.bas | error | `` | error | `` | true |
| on_error_goto_label_resume.bas | ok | `100` | ok | `6` | false |
| on_error_goto_label_then_goto_zero_error.bas | error | `` | error | `` | true |
| on_error_goto_zero_fail.bas | error | `` | error | `` | true |
| on_error_resume_continue.bas | ok | `2` | ok | `2` | true |
| on_error_resume_next.bas | ok | `5` | ok | `5` | true |
| operator_arithmetic_object_plus_error.bas | error | `` | error | `` | true |
| operator_comparison_object_long_error.bas | error | `` | error | `` | true |
| option_explicit_error.bas | error | `` | error | `` | true |
| option_explicit_ok.bas | ok | `3` | ok | `3` | true |
| params_byref_error.bas | ok | `` | error | `` | false |
| params_byref.bas | ok | `2` | ok | `2` | true |
| params_byval.bas | ok | `1` | ok | `1` | true |
| params_named_bind.bas | ok | `9` | ok | `9` | true |
| params_named_optional_omit.bas | ok | `7` | ok | `7` | true |
| params_named_positional_after_named_error.bas | error | `` | error | `` | true |
| params_optional_default.bas | ok | `7` | ok | `7` | true |
| params_optional_override.bas | ok | `9` | ok | `9` | true |
| params_paramarray_dispatch_boundary.bas | skip | `` | ok | `25008` |  |
| params_paramarray_empty.bas | ok | `-1` | ok | `-1` | true |
| params_paramarray_named_error.bas | error | `` | error | `` | true |
| params_paramarray_pack.bas | ok | `2` | ok | `2` | true |
| proc_call_chain.bas | ok | `1` | ok | `1` | true |
| proc_call_local_scope.bas | ok | `2` | ok | `2` | true |
| project_model_implements_requires_class_graph.bas | error | `` | error | `` | true |
| project_model_raiseevent_requires_class_graph.bas | error | `` | error | `` | true |
| project_model_withevents_requires_class_graph.bas | error | `` | error | `` | true |
| property_get_declaration_basic.bas | ok | `4` | ok | `4` | true |
| property_get_expression_basic.bas | ok | `9` | ok | `0` | false |
| property_let_byref_route.bas | ok | `1` | ok | `3` | false |
| property_set_byref_route.bas | error | `` | ok | `7` | false |
| redim_expand_allows_new_index.bas | error | `` | ok | `0,0,5,0,5` | false |
| redim_preserve_illegal_non_last_dim_error.bas | error | `` | error | `` | true |
| redim_preserve_keeps_values.bas | error | `` | ok | `7,0,7,0,0` | false |
| redim_preserve_multidim_last_dimension.bas | error | `` | ok | `7,0,0,0,7,0,0` | false |
| redim_preserve_shrink_expand_clears_tail.bas | error | `` | ok | `0,0,0,0,0` | false |
| redim_shrink_bounds_error.bas | error | `` | error | `` | true |
| redim_without_preserve_resets.bas | error | `` | ok | `0,0,0,0,0` | false |
| regression_cverr_error_resume_bridge.bas | ok | `11,20,0,0,0` | ok | `11,0,1,10,0` | false |
| regression_cverr_predicate_domain.bas | ok | `1,1,0,0,0` | ok | `1,1,0,0,0` | true |
| resume_label_basic.bas | ok | `6` | ok | `6` | true |
| resume_next_statement_ok.bas | ok | `1` | ok | `1` | true |
| resume_statement_basic.bas | ok | `2` | ok | `2` | true |
| select_case_basic.bas | ok | `20` | ok | `20` | true |
| select_case_else.bas | ok | `99` | ok | `99` | true |
| select_case_is_range.bas | ok | `22` | ok | `22` | true |
| select_case_multi.bas | ok | `30` | ok | `30` | true |
| smoke.bas | ok | `15` | ok | `15` | true |
| stdlib_advanced_instrrev_like.bas | ok | `4,1` | ok | `4,1` | true |
| stdlib_advanced_replace_trim.bas | ok | `16745,456,321` | ok | `16745,456,321` | true |
| stdlib_advanced_split_join.bas | error | `` | ok | `3,789` | false |
| stdlib_advanced_strcomp.bas | ok | `-1,0` | ok | `-1,0` | true |
| stdlib_array_introspection_bounds.bas | ok | `0,0,2` | ok | `-999999997,0,2` | false |
| stdlib_array_introspection_types.bas | ok | `0,8204,0` | ok | `-999999998,8204,1001` | false |
| stdlib_date_add_diff.bas | error | `` | ok | `20260231,3` | false |
| stdlib_date_serial_value.bas | ok | `46081,46081` | ok | `20260228,20260228` | false |
| stdlib_datetime_expansion.bas | error | `` | ok | `20260301,123456,20260301,20240203,20240203,20240203,20240203,2,42` | false |
| stdlib_error_cverr_identity.bas | ok | `-899999983` | ok | `-899999983` | true |
| stdlib_error_err_raise_fail.bas | error | `` | error | `` | true |
| stdlib_error_err_raise_resume.bas | ok | `11` | ok | `11` | true |
| stdlib_file_stub_intrinsics.bas | error | `` | ok | `1,3,4,5` | false |
| stdlib_financial_zero_rate.bas | ok | `-11,-11,-3` | ok | `-11,-11,-3` | true |
| stdlib_format_core.bas | ok | `12345,1` | ok | `12345,678` | false |
| stdlib_host_sensitive_mix.bas | skip | `` | ok | `1,4` |  |
| stdlib_host_sensitive_shell_environ_dir.bas | skip | `` | ok | `1,9,1` |  |
| stdlib_host_sensitive_zero_fallback.bas | skip | `` | ok | `0,0` |  |
| stdlib_instr_case_ops.bas | ok | `3,789,654` | ok | `3,789,654` | true |
| stdlib_introspection_expansion.bas | error | `` | ok | `1,1,1,1` | false |
| stdlib_len_basic.bas | error | `` | ok | `4` | false |
| stdlib_math_primitives.bas | error | `` | ok | `7,-1,9,20` | false |
| stdlib_math_transcendental_identity.bas | ok | `0,1,0,1` | ok | `0,1,0,1` | true |
| stdlib_numeric_expansion.bas | ok | `0,21,1,-226` | ok | `31,17,9,11` | false |
| stdlib_random_financial_expansion.bas | error | `` | ok | `1,42,59,-50,-28,-99,-38` | false |
| stdlib_slice_ops.bas | ok | `12,45,234` | ok | `12,45,234` | true |
| stdlib_string_expansion_core.bas | ok | `0,0,0,66,777` | ok | `4,3,66,66,777` | false |
| stdlib_time_serial_value.bas | ok | `0,0` | ok | `3723,3723` | false |
| stdlib_variant_predicates.bas | ok | `1,1,0` | ok | `1,1,0` | true |
| string_compare_option_binary.bas | ok | `3,-1` | ok | `3,-1` | true |
| string_compare_option_text.bas | ok | `3,-1` | ok | `3,-1` | true |
| string_join_array_tag_count.bas | ok | `0,10203` | ok | `-999999997,3` | false |
| string_mid_statement_mutation.bas | ok | `19945` | ok | `19945` | true |
| string_slice_ops_dollar.bas | ok | `12,45,234` | ok | `12,45,234` | true |
| string_vbnullstring_basic.bas | ok | `0` | ok | `0` | true |
| string_vbnullstring_long_error.bas | error | `` | error | `` | true |
| string_vbnullstring_object_error.bas | error | `` | error | `` | true |
| string_vbnullstring_predicates.bas | ok | `0,0,0,0` | ok | `0,1,0,0` | false |
| subtract.bas | ok | `16` | ok | `16` | true |
| typechar_explicit_as_precedence_error.bas | error | `` | error | `` | true |
| typed_fastpath_hotloop.bas | ok | `300,101` | ok | `300,101` | true |
| typeof_is_condition_basic.bas | error | `` | ok | `1` | false |
| udt_declaration_basic.bas | ok | `9` | ok | `9` | true |
| udt_field_access_basic.bas | error | `` | ok | `0,7,7,7` | false |
| udt_whole_assignment_copy.bas | error | `` | ok | `0,7,9,0,7,9,9` | false |
| udt_whole_assignment_overwrite.bas | error | `` | ok | `0,7,6,0,7,6` | false |
| while_wend_basic.bas | ok | `3` | ok | `3` | true |
| with_block_basic.bas | error | `` | ok | `3,3` | false |
| with_block_member_target_chain.bas | error | `` | ok | `7,7` | false |
