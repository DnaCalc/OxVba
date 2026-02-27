# Formal Proof Inventory

This inventory tracks formal artifacts and proof-adjacent harnesses by profile.

## v2 (`mvp-controlflow-v2`)

- FO-V2-001
  - Area: VM control-flow safety
  - Artifact: `crates/oxvba-vm/src/interpreter.rs`
  - Harness: `pc_progression_is_safe_for_valid_jump_target`
- FO-V2-002
  - Area: Compiler temp-slot safety
  - Artifact: `crates/oxvba-compiler/src/emit.rs`
  - Harness: `temp_slots_do_not_overlap_declared_slots`

## v3 (`mvp-formal-foundation-v3`)

- FO-V3-001
  - Area: Formal tooling availability
  - Artifact: `scripts/run-formal.ps1`
  - Command: `cargo kani --version`

## v4 (`mvp-boolean-logic-v4`)

- FO-V4-001
  - Area: comparator output invariants
  - Artifact: `crates/oxvba-vm/src/interpreter.rs`
  - Harness: `comparator_ops_produce_boolean_values`

## v5 (`mvp-else-paths-v5`)

- FO-V5-001
  - Area: branch totality
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v5_branch_selection_is_total_over_small_domain`
- FO-V5-002
  - Area: branch model equivalence
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v5_branch_selection_matches_reference_model`
- FO-V5-003
  - Area: single-path write effect
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v5_no_dual_branch_write_effect`

## v6 (`mvp-while-loop-v6`)

- FO-V6-001
  - Area: pre-condition loop model
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v6_do_while_matches_reference_model`
- FO-V6-002
  - Area: post-condition loop model
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v6_post_condition_loop_semantics`
- FO-V6-003
  - Area: `Exit Do` short-circuit behavior
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v6_exit_do_short_circuits_iteration`

## v7 (`mvp-select-case-v7`)

- FO-V7-001
  - Area: first-match case determinism
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v7_select_case_first_match_wins`
- FO-V7-002
  - Area: `Case Else` fallback semantics
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v7_select_case_else_fallback`
- FO-V7-003
  - Area: multi-value arm inclusion semantics
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v7_select_case_multi_value_arm`

## v8 (`mvp-procedures-v8`)

- FO-V8-001
  - Area: caller return progression
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v8_call_returns_to_caller`
- FO-V8-002
  - Area: procedure-local slot isolation
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v8_local_scope_isolated_between_procedures`
- FO-V8-003
  - Area: nested call chain integrity
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v8_nested_call_chain_integrity`

## v9 (`mvp-params-v9`)

- FO-V9-001
  - Area: `ByVal` mutation isolation
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v9_byval_does_not_propagate_mutation`
- FO-V9-002
  - Area: `ByRef` mutation propagation
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v9_byref_propagates_mutation`
- FO-V9-003
  - Area: `ByRef` argument validity rule
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v9_byref_requires_variable_argument`

## v10 (`mvp-arrays-v10`)

- FO-V10-001
  - Area: array store/load roundtrip
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v10_array_store_load_roundtrip`
- FO-V10-002
  - Area: array bounds rejection
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v10_array_bounds_violation_errors`
- FO-V10-003
  - Area: zero-index array semantics
  - Artifact: `crates/oxvba-host/src/engine.rs`
  - Harness: `formal_v10_array_index_zero_is_valid`

## Reports

- Latest markdown report: `docs/evidence/formal/latest_run.md`
- Latest csv report: `docs/evidence/formal/latest_run.csv`
- Extended backlog: `docs/evidence/formal/EXTENDED_TODO.md`
