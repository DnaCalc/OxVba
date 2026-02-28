# Formal Run Report

- Timestamp (UTC): 2026-02-28T00:48:53Z
- Profile scope: mvp-stdlib-string-advanced-v47
- Overall mode: non-blocking
- Kani required: false
- Kani execution: deferred-to-wsl-async
- cargo-kani (local): unavailable
- cargo-kani (wsl): cargo-kani 0.67.0

| Obligation | Profile | Blocking | Status | Command | Artifact | Note |
|---|---|---|---|---|---|---|
| FO-V2-001 | v2 | no | skipped | cargo kani -p oxvba-vm --harness pc_progression_is_safe_for_valid_jump_target | crates/oxvba-vm/src/interpreter.rs | cargo-kani available via WSL; rerun with -UseWslKani (recommended via run-formal-kani-async.ps1) |
| FO-V2-002 | v2 | no | skipped | cargo kani -p oxvba-compiler --harness temp_slots_do_not_overlap_declared_slots | crates/oxvba-compiler/src/emit.rs | cargo-kani available via WSL; rerun with -UseWslKani (recommended via run-formal-kani-async.ps1) |
| FO-V3-001 | v3 | no | skipped | cargo kani --version | scripts/run-formal.ps1 | cargo-kani available via WSL; rerun with -UseWslKani (recommended via run-formal-kani-async.ps1) |
| FO-V4-001 | v4 | no | skipped | cargo kani -p oxvba-vm --harness comparator_ops_produce_boolean_values | crates/oxvba-vm/src/interpreter.rs | cargo-kani available via WSL; rerun with -UseWslKani (recommended via run-formal-kani-async.ps1) |
| FO-V5-001 | v5 | no | pass | cargo test -p oxvba-host formal_v5_branch_selection_is_total_over_small_domain | crates/oxvba-host/src/engine.rs |  |
| FO-V5-002 | v5 | no | pass | cargo test -p oxvba-host formal_v5_branch_selection_matches_reference_model | crates/oxvba-host/src/engine.rs |  |
| FO-V5-003 | v5 | no | pass | cargo test -p oxvba-host formal_v5_no_dual_branch_write_effect | crates/oxvba-host/src/engine.rs |  |
| FO-V6-001 | v6 | no | pass | cargo test -p oxvba-host formal_v6_do_while_matches_reference_model | crates/oxvba-host/src/engine.rs |  |
| FO-V6-002 | v6 | no | pass | cargo test -p oxvba-host formal_v6_post_condition_loop_semantics | crates/oxvba-host/src/engine.rs |  |
| FO-V6-003 | v6 | no | pass | cargo test -p oxvba-host formal_v6_exit_do_short_circuits_iteration | crates/oxvba-host/src/engine.rs |  |
| FO-V7-001 | v7 | no | pass | cargo test -p oxvba-host formal_v7_select_case_first_match_wins | crates/oxvba-host/src/engine.rs |  |
| FO-V7-002 | v7 | no | pass | cargo test -p oxvba-host formal_v7_select_case_else_fallback | crates/oxvba-host/src/engine.rs |  |
| FO-V7-003 | v7 | no | pass | cargo test -p oxvba-host formal_v7_select_case_multi_value_arm | crates/oxvba-host/src/engine.rs |  |
| FO-V8-001 | v8 | no | pass | cargo test -p oxvba-host formal_v8_call_returns_to_caller | crates/oxvba-host/src/engine.rs |  |
| FO-V8-002 | v8 | no | pass | cargo test -p oxvba-host formal_v8_local_scope_isolated_between_procedures | crates/oxvba-host/src/engine.rs |  |
| FO-V8-003 | v8 | no | pass | cargo test -p oxvba-host formal_v8_nested_call_chain_integrity | crates/oxvba-host/src/engine.rs |  |
| FO-V9-001 | v9 | no | pass | cargo test -p oxvba-host formal_v9_byval_does_not_propagate_mutation | crates/oxvba-host/src/engine.rs |  |
| FO-V9-002 | v9 | no | pass | cargo test -p oxvba-host formal_v9_byref_propagates_mutation | crates/oxvba-host/src/engine.rs |  |
| FO-V9-003 | v9 | no | pass | cargo test -p oxvba-host formal_v9_byref_requires_variable_argument | crates/oxvba-host/src/engine.rs |  |
| FO-V10-001 | v10 | no | pass | cargo test -p oxvba-host formal_v10_array_store_load_roundtrip | crates/oxvba-host/src/engine.rs |  |
| FO-V10-002 | v10 | no | pass | cargo test -p oxvba-host formal_v10_array_bounds_violation_errors | crates/oxvba-host/src/engine.rs |  |
| FO-V10-003 | v10 | no | pass | cargo test -p oxvba-host formal_v10_array_index_zero_is_valid | crates/oxvba-host/src/engine.rs |  |
| FO-V11-001 | v11 | no | pass | cargo test -p oxvba-host formal_v11_resume_next_records_error_number | crates/oxvba-host/src/engine.rs |  |
| FO-V11-002 | v11 | no | pass | cargo test -p oxvba-host formal_v11_default_error_mode_fails | crates/oxvba-host/src/engine.rs |  |
| FO-V11-003 | v11 | no | pass | cargo test -p oxvba-host formal_v11_resume_next_continues_execution | crates/oxvba-host/src/engine.rs |  |
| FO-V12-001 | v12 | no | pass | cargo test -p oxvba-host formal_v12_on_error_goto_zero_restores_fail_behavior | crates/oxvba-host/src/engine.rs |  |
| FO-V12-002 | v12 | no | pass | cargo test -p oxvba-host formal_v12_resume_next_statement_no_panic | crates/oxvba-host/src/engine.rs |  |
| FO-V12-003 | v12 | no | pass | cargo test -p oxvba-host formal_v12_resume_next_then_continue_updates_value | crates/oxvba-host/src/engine.rs |  |
| FO-V13-001 | v13 | no | pass | cargo test -p oxvba-host formal_v13_variant_numeric_coercion_long_to_double | crates/oxvba-host/src/engine.rs |  |
| FO-V13-002 | v13 | no | pass | cargo test -p oxvba-host formal_v13_variant_numeric_bool_to_long | crates/oxvba-host/src/engine.rs |  |
| FO-V13-003 | v13 | no | pass | cargo test -p oxvba-host formal_v13_variant_numeric_addition_consistency | crates/oxvba-host/src/engine.rs |  |
| FO-V14-001 | v14 | no | pass | cargo test -p oxvba-host formal_v14_bstr_roundtrip_ascii | crates/oxvba-host/src/engine.rs |  |
| FO-V14-002 | v14 | no | pass | cargo test -p oxvba-host formal_v14_bstr_concat_law | crates/oxvba-host/src/engine.rs |  |
| FO-V14-003 | v14 | no | pass | cargo test -p oxvba-host formal_v14_bstr_empty_identity | crates/oxvba-host/src/engine.rs |  |
| FO-V15-001 | v15 | no | pass | cargo test -p oxvba-host formal_v15_date_currency_projection_is_stable | crates/oxvba-host/src/engine.rs |  |
| FO-V15-002 | v15 | no | pass | cargo test -p oxvba-host formal_v15_currency_scale_roundtrip | crates/oxvba-host/src/engine.rs |  |
| FO-V15-003 | v15 | no | pass | cargo test -p oxvba-host formal_v15_date_addition_monotonicity | crates/oxvba-host/src/engine.rs |  |
| FO-V16-001 | v16 | no | pass | cargo test -p oxvba-host formal_v16_spec_trace_matches_runtime_small_program | crates/oxvba-host/src/engine.rs |  |
| FO-V16-002 | v16 | no | pass | cargo test -p oxvba-host formal_v16_spec_trace_matches_branch_program | crates/oxvba-host/src/engine.rs |  |
| FO-V16-003 | v16 | no | pass | cargo test -p oxvba-host formal_v16_trace_format_is_csv_stable | crates/oxvba-host/src/engine.rs |  |
| FO-V17-001 | v17 | no | pass | cargo test -p oxvba-host formal_v17_formal_manifest_has_active_entries | crates/oxvba-host/src/engine.rs |  |
| FO-V17-002 | v17 | no | pass | cargo test -p oxvba-host formal_v17_runner_script_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V17-003 | v17 | no | pass | cargo test -p oxvba-host formal_v17_meta_check_includes_formal_switch | crates/oxvba-host/src/engine.rs |  |
| FO-V18-001 | v18 | no | pass | cargo test -p oxvba-host formal_v18_divergence_index_is_present | crates/oxvba-host/src/engine.rs |  |
| FO-V18-002 | v18 | no | pass | cargo test -p oxvba-host formal_v18_divergence_records_have_scope_lines | crates/oxvba-host/src/engine.rs |  |
| FO-V18-003 | v18 | no | pass | cargo test -p oxvba-host formal_v18_divergence_records_link_evidence | crates/oxvba-host/src/engine.rs |  |
| FO-V19-001 | v19 | no | pass | cargo test -p oxvba-compiler formal_v19_noop_assignments_removed | crates/oxvba-compiler/src/optimize.rs |  |
| FO-V19-002 | v19 | no | pass | cargo test -p oxvba-compiler formal_v19_optimizer_preserves_non_noop_assignments | crates/oxvba-compiler/src/optimize.rs |  |
| FO-V19-003 | v19 | no | pass | cargo test -p oxvba-compiler formal_v19_constant_if_eliminates_unreachable_branch | crates/oxvba-compiler/src/optimize.rs |  |
| FO-V19-004 | v19 | no | pass | cargo test -p oxvba-compiler formal_v19_constant_select_case_is_folded | crates/oxvba-compiler/src/optimize.rs |  |
| FO-V19-005 | v19 | no | pass | cargo test -p oxvba-compiler formal_v19_for_loop_semantics_preserved_with_optimized_body | crates/oxvba-compiler/src/optimize.rs |  |
| FO-V20-001 | v20 | no | pass | cargo test -p oxvba-host formal_v20_jit_vm_equivalence_arithmetic | crates/oxvba-host/src/engine.rs |  |
| FO-V20-002 | v20 | no | pass | cargo test -p oxvba-host formal_v20_jit_vm_equivalence_control_flow | crates/oxvba-host/src/engine.rs |  |
| FO-V20-003 | v20 | no | pass | cargo test -p oxvba-host formal_v20_jit_vm_equivalence_error_state | crates/oxvba-host/src/engine.rs |  |
| FO-V21-001 | v21 | no | pass | cargo test -p oxvba-host formal_v21_opt_toggle_parity | crates/oxvba-host/src/engine.rs |  |
| FO-V21-002 | v21 | no | pass | cargo test -p oxvba-host formal_v21_jit_vm_guardrail_equivalence | crates/oxvba-host/src/engine.rs |  |
| FO-V21-003 | v21 | no | pass | cargo test -p oxvba-host formal_v21_benchmark_script_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V22-001 | v22 | no | pass | cargo test -p oxvba-host formal_v22_jit_vm_equivalence_for_loop_backedge | crates/oxvba-host/src/engine.rs |  |
| FO-V22-002 | v22 | no | pass | cargo test -p oxvba-host formal_v22_jit_vm_equivalence_do_loop_backedge | crates/oxvba-host/src/engine.rs |  |
| FO-V22-003 | v22 | no | pass | cargo test -p oxvba-host formal_v22_cranelift_supports_loop_subset | crates/oxvba-host/src/engine.rs |  |
| FO-V23-001 | v23 | no | pass | cargo test -p oxvba-host formal_v23_formal_runner_has_require_kani_switch | crates/oxvba-host/src/engine.rs |  |
| FO-V23-002 | v23 | no | pass | cargo test -p oxvba-host formal_v23_setup_kani_script_documents_bootstrap | crates/oxvba-host/src/engine.rs |  |
| FO-V23-003 | v23 | no | pass | cargo test -p oxvba-host formal_v23_ci_supports_optional_kani_job | crates/oxvba-host/src/engine.rs |  |
| FO-V24-001 | v24 | no | pass | cargo test -p oxvba-host formal_v24_jit_vm_equivalence_call_subset | crates/oxvba-host/src/engine.rs |  |
| FO-V24-002 | v24 | no | pass | cargo test -p oxvba-host formal_v24_cranelift_supports_call_subset | crates/oxvba-host/src/engine.rs |  |
| FO-V24-003 | v24 | no | pass | cargo test -p oxvba-host formal_v24_jit_falls_back_for_error_state_subset | crates/oxvba-host/src/engine.rs |  |
| FO-V25-001 | v25 | no | pass | cargo test -p oxvba-host formal_v25_optimizer_parity_on_constant_if_fold | crates/oxvba-host/src/engine.rs |  |
| FO-V25-002 | v25 | no | pass | cargo test -p oxvba-host formal_v25_optimizer_parity_on_select_case_fold | crates/oxvba-host/src/engine.rs |  |
| FO-V25-003 | v25 | no | pass | cargo test -p oxvba-host formal_v25_optimizer_parity_on_dead_store_reduction | crates/oxvba-host/src/engine.rs |  |
| FO-V26-001 | v26 | no | pass | cargo test -p oxvba-host formal_v26_script_defaults_target_v26_profile_scope | crates/oxvba-host/src/engine.rs |  |
| FO-V26-002 | v26 | no | pass | cargo test -p oxvba-host formal_v26_benchmark_default_targets_v26_artifact | crates/oxvba-host/src/engine.rs |  |
| FO-V26-003 | v26 | no | pass | cargo test -p oxvba-host formal_v26_profile_status_document_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V27-001 | v27 | no | pass | cargo test -p oxvba-host formal_v27_async_runner_supports_full_action_set | crates/oxvba-host/src/engine.rs |  |
| FO-V27-002 | v27 | no | pass | cargo test -p oxvba-host formal_v27_async_runner_uses_hidden_background_window | crates/oxvba-host/src/engine.rs |  |
| FO-V27-003 | v27 | no | pass | cargo test -p oxvba-host formal_v27_async_runner_persists_state_and_exit_markers | crates/oxvba-host/src/engine.rs |  |
| FO-V28-001 | v28 | no | pass | cargo test -p oxvba-host formal_v28_vm_pc_progression_kani_harness_is_bounded | crates/oxvba-host/src/engine.rs |  |
| FO-V28-002 | v28 | no | pass | cargo test -p oxvba-host formal_v28_vm_jump_helper_has_regression_unit_test | crates/oxvba-host/src/engine.rs |  |
| FO-V28-003 | v28 | no | pass | cargo test -p oxvba-host formal_v28_profile_status_document_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V29-001 | v29 | no | pass | cargo test -p oxvba-host formal_v29_async_runner_wait_supports_timeouts | crates/oxvba-host/src/engine.rs |  |
| FO-V29-002 | v29 | no | pass | cargo test -p oxvba-host formal_v29_capacity_workset_document_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V29-003 | v29 | no | pass | cargo test -p oxvba-host formal_v29_obligation_entries_are_registered | crates/oxvba-host/src/engine.rs |  |
| FO-V30-001 | v30 | no | pass | cargo test -p oxvba-host formal_v30_variant_layout_uses_com_reserved_fields | crates/oxvba-host/src/engine.rs |  |
| FO-V30-002 | v30 | no | pass | cargo test -p oxvba-host formal_v30_variant_runtime_has_com_layout_shape_test | crates/oxvba-host/src/engine.rs |  |
| FO-V30-003 | v30 | no | pass | cargo test -p oxvba-host formal_v30_profile_status_document_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V31-001 | v31 | no | pass | cargo test -p oxvba-host formal_v31_variant_wire_roundtrip_helpers_exist | crates/oxvba-host/src/engine.rs |  |
| FO-V31-002 | v31 | no | pass | cargo test -p oxvba-host formal_v31_boundary_marshalling_workset_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V31-003 | v31 | no | pass | cargo test -p oxvba-host formal_v31_profile_status_document_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V32-001 | v32 | no | pass | cargo test -p oxvba-host formal_v32_language_coverage_index_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V32-002 | v32 | no | pass | cargo test -p oxvba-host formal_v32_meta_check_validates_language_coverage | crates/oxvba-host/src/engine.rs |  |
| FO-V32-003 | v32 | no | pass | cargo test -p oxvba-host formal_v32_language_coverage_status_taxonomy_is_present | crates/oxvba-host/src/engine.rs |  |
| FO-V33-001 | v33 | no | pass | cargo test -p oxvba-host formal_v33_core_coverage_tracks_key_control_flow_constructs | crates/oxvba-host/src/engine.rs |  |
| FO-V33-002 | v33 | no | pass | cargo test -p oxvba-host formal_v33_core_coverage_workset_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V33-003 | v33 | no | pass | cargo test -p oxvba-host formal_v33_core_conformance_fixtures_are_present | crates/oxvba-host/src/engine.rs |  |
| FO-V34-001 | v34 | no | pass | cargo test -p oxvba-host formal_v34_object_coverage_entries_are_present | crates/oxvba-host/src/engine.rs |  |
| FO-V34-002 | v34 | no | pass | cargo test -p oxvba-host formal_v34_object_coverage_workset_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V34-003 | v34 | no | pass | cargo test -p oxvba-host formal_v34_profile_status_document_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V35-001 | v35 | no | pass | cargo test -p oxvba-host formal_v35_hotpath_workset_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V35-002 | v35 | no | pass | cargo test -p oxvba-host formal_v35_jit_vm_hotpath_parity_examples_exist | crates/oxvba-host/src/engine.rs |  |
| FO-V35-003 | v35 | no | pass | cargo test -p oxvba-host formal_v35_profile_status_document_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V36-001 | v36 | no | pass | cargo test -p oxvba-host formal_v36_script_defaults_target_v36_profile_scope | crates/oxvba-host/src/engine.rs |  |
| FO-V36-002 | v36 | no | pass | cargo test -p oxvba-host formal_v36_benchmark_default_targets_v36_artifact | crates/oxvba-host/src/engine.rs |  |
| FO-V36-003 | v36 | no | pass | cargo test -p oxvba-host formal_v36_profile_status_document_exists | crates/oxvba-host/src/engine.rs |  |
| FO-V37-001 | v37 | no | pass | cargo test -p oxvba-host formal_v37_optional_param_default_applies_when_omitted | crates/oxvba-host/src/engine.rs |  |
| FO-V37-002 | v37 | no | pass | cargo test -p oxvba-host formal_v37_optional_param_explicit_value_overrides_default | crates/oxvba-host/src/engine.rs |  |
| FO-V37-003 | v37 | no | pass | cargo test -p oxvba-host formal_v37_optional_param_missing_required_arg_is_rejected | crates/oxvba-host/src/engine.rs |  |
| FO-V38-001 | v38 | no | pass | cargo test -p oxvba-host formal_v38_named_args_bind_by_parameter_name | crates/oxvba-host/src/engine.rs |  |
| FO-V38-002 | v38 | no | pass | cargo test -p oxvba-host formal_v38_named_args_allow_omitting_optional_by_name | crates/oxvba-host/src/engine.rs |  |
| FO-V38-003 | v38 | no | pass | cargo test -p oxvba-host formal_v38_named_args_reject_positional_after_named | crates/oxvba-host/src/engine.rs |  |
| FO-V40-001 | v40 | no | pass | cargo test -p oxvba-host formal_v40_gosub_executes_label_body_and_returns | crates/oxvba-host/src/engine.rs |  |
| FO-V40-002 | v40 | no | pass | cargo test -p oxvba-host formal_v40_gosub_missing_label_is_rejected | crates/oxvba-host/src/engine.rs |  |
| FO-V40-003 | v40 | no | pass | cargo test -p oxvba-host formal_v40_gosub_return_stack_handles_repeated_calls | crates/oxvba-host/src/engine.rs |  |
| FO-V41-001 | v41 | no | pass | cargo test -p oxvba-host formal_v41_on_error_goto_label_jumps_to_handler | crates/oxvba-host/src/engine.rs |  |
| FO-V41-002 | v41 | no | pass | cargo test -p oxvba-host formal_v41_on_error_goto_label_missing_target_is_rejected | crates/oxvba-host/src/engine.rs |  |
| FO-V41-003 | v41 | no | pass | cargo test -p oxvba-host formal_v41_on_error_goto_zero_disables_label_handler | crates/oxvba-host/src/engine.rs |  |
| FO-V42-001 | v42 | no | pass | cargo test -p oxvba-host formal_v42_redim_preserve_retains_existing_values | crates/oxvba-host/src/engine.rs |  |
| FO-V42-002 | v42 | no | pass | cargo test -p oxvba-host formal_v42_redim_without_preserve_reinitializes_array | crates/oxvba-host/src/engine.rs |  |
| FO-V42-003 | v42 | no | pass | cargo test -p oxvba-host formal_v42_redim_shrink_rejects_out_of_bounds_access | crates/oxvba-host/src/engine.rs |  |
| FO-V43-001 | v43 | no | pass | cargo test -p oxvba-host formal_v43_module_const_evaluates_in_expression | crates/oxvba-host/src/engine.rs |  |
| FO-V43-002 | v43 | no | pass | cargo test -p oxvba-host formal_v43_enum_members_bind_to_expected_values | crates/oxvba-host/src/engine.rs |  |
| FO-V43-003 | v43 | no | pass | cargo test -p oxvba-host formal_v43_udt_declaration_block_is_parse_tolerated | crates/oxvba-host/src/engine.rs |  |
| FO-V44-001 | v44 | no | pass | cargo test -p oxvba-host formal_v44_property_let_routes_assignment_byref | crates/oxvba-host/src/engine.rs |  |
| FO-V44-002 | v44 | no | pass | cargo test -p oxvba-host formal_v44_property_set_routes_assignment_byref | crates/oxvba-host/src/engine.rs |  |
| FO-V44-003 | v44 | no | pass | cargo test -p oxvba-host formal_v44_property_get_block_is_parse_tolerated | crates/oxvba-host/src/engine.rs |  |
| FO-V45-001 | v45 | no | pass | cargo test -p oxvba-host formal_v45_cint_conversion_subset | crates/oxvba-host/src/engine.rs |  |
| FO-V45-002 | v45 | no | pass | cargo test -p oxvba-host formal_v45_nested_conversion_subset | crates/oxvba-host/src/engine.rs |  |
| FO-V45-003 | v45 | no | pass | cargo test -p oxvba-host formal_v45_val_str_conversion_subset | crates/oxvba-host/src/engine.rs |  |
| FO-V46-001 | v46 | no | pass | cargo test -p oxvba-host formal_v46_len_intrinsic_digit_count | crates/oxvba-host/src/engine.rs |  |
| FO-V46-002 | v46 | no | pass | cargo test -p oxvba-host formal_v46_slice_intrinsics_digit_subsets | crates/oxvba-host/src/engine.rs |  |
| FO-V46-003 | v46 | no | pass | cargo test -p oxvba-host formal_v46_instr_and_case_intrinsics | crates/oxvba-host/src/engine.rs |  |
| FO-V47-001 | v47 | no | pass | cargo test -p oxvba-host formal_v47_split_and_join_intrinsics | crates/oxvba-host/src/engine.rs |  |
| FO-V47-002 | v47 | no | pass | cargo test -p oxvba-host formal_v47_replace_and_trim_intrinsics | crates/oxvba-host/src/engine.rs |  |
| FO-V47-003 | v47 | no | pass | cargo test -p oxvba-host formal_v47_strcomp_intrinsic_subset | crates/oxvba-host/src/engine.rs |  |
