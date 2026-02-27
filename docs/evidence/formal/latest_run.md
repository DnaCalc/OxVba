# Formal Run Report

- Timestamp (UTC): 2026-02-27T13:21:00Z
- Profile scope: mvp-perf-stabilization-v21
- Overall mode: non-blocking
- cargo-kani: unavailable

| Obligation | Profile | Blocking | Status | Command | Artifact | Note |
|---|---|---|---|---|---|---|
| FO-V2-001 | v2 | no | skipped | cargo kani -p oxvba-vm --harness pc_progression_is_safe_for_valid_jump_target | crates/oxvba-vm/src/interpreter.rs | cargo-kani not available |
| FO-V2-002 | v2 | no | skipped | cargo kani -p oxvba-compiler --harness temp_slots_do_not_overlap_declared_slots | crates/oxvba-compiler/src/emit.rs | cargo-kani not available |
| FO-V3-001 | v3 | no | skipped | cargo kani --version | scripts/run-formal.ps1 | cargo-kani not available |
| FO-V4-001 | v4 | no | skipped | cargo kani -p oxvba-vm --harness comparator_ops_produce_boolean_values | crates/oxvba-vm/src/interpreter.rs | cargo-kani not available |
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
| FO-V19-003 | v19 | no | pass | cargo test -p oxvba-compiler formal_v19_nested_blocks_optimized_safely | crates/oxvba-compiler/src/optimize.rs |  |
| FO-V20-001 | v20 | no | pass | cargo test -p oxvba-host formal_v20_jit_vm_equivalence_arithmetic | crates/oxvba-host/src/engine.rs |  |
| FO-V20-002 | v20 | no | pass | cargo test -p oxvba-host formal_v20_jit_vm_equivalence_control_flow | crates/oxvba-host/src/engine.rs |  |
| FO-V20-003 | v20 | no | pass | cargo test -p oxvba-host formal_v20_jit_vm_equivalence_error_state | crates/oxvba-host/src/engine.rs |  |
| FO-V21-001 | v21 | no | pass | cargo test -p oxvba-host formal_v21_opt_toggle_parity | crates/oxvba-host/src/engine.rs |  |
| FO-V21-002 | v21 | no | pass | cargo test -p oxvba-host formal_v21_jit_vm_guardrail_equivalence | crates/oxvba-host/src/engine.rs |  |
| FO-V21-003 | v21 | no | pass | cargo test -p oxvba-host formal_v21_benchmark_script_exists | crates/oxvba-host/src/engine.rs |  |
