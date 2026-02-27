# Formal Run Report

- Timestamp (UTC): 2026-02-27T12:47:33Z
- Profile scope: mvp-params-v9
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
