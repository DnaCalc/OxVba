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

## Reports

- Latest markdown report: `docs/evidence/formal/latest_run.md`
- Latest csv report: `docs/evidence/formal/latest_run.csv`
- Extended backlog: `docs/evidence/formal/EXTENDED_TODO.md`
