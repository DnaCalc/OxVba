# Formal Run Report

- Timestamp (UTC): 2026-02-27T12:25:10Z
- Profile scope: mvp-while-loop-v6
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
