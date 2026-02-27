# Formal Obligation Manifest

This manifest tracks profile-scoped formal obligations.

## Status Legend
- `pass`: obligation currently passing.
- `todo`: obligation currently failing or not executable; non-blocking at this stage.
- `skipped`: tooling unavailable or intentionally skipped.

## Active Obligations

Source of truth:
- `docs/evidence/formal/obligations.csv`

| Obligation ID | Profile | Area | Harness/Proof | Runner | Blocking | Current Status | Status Source |
|---|---|---|---|---|---|---|---|
| FO-V2-001 | v2 (`mvp-controlflow-v2`) | VM control-flow safety | `pc_progression_is_safe_for_valid_jump_target` | `scripts/run-formal.ps1` | no | todo (tooling unavailable) | `docs/evidence/formal/latest_run.md` |
| FO-V2-002 | v2 (`mvp-controlflow-v2`) | Compiler temp-slot safety | `temp_slots_do_not_overlap_declared_slots` | `scripts/run-formal.ps1` | no | todo (tooling unavailable) | `docs/evidence/formal/latest_run.md` |
| FO-V3-001 | v3 (`mvp-formal-foundation-v3`) | Formal tooling availability | `cargo kani --version` | `scripts/run-formal.ps1` | no | todo (tooling unavailable) | `docs/evidence/formal/latest_run.md` |
| FO-V4-001 | v4 (`mvp-boolean-logic-v4`) | Comparator output invariants | `comparator_ops_produce_boolean_values` | `scripts/run-formal.ps1` | no | todo (tooling unavailable) | `docs/evidence/formal/latest_run.md` |
| FO-V5-001 | v5 (`mvp-else-paths-v5`) | Branch totality (small domain) | `formal_v5_branch_selection_is_total_over_small_domain` | `scripts/run-formal.ps1` | no | todo (pending latest run update) | `docs/evidence/formal/latest_run.md` |
| FO-V5-002 | v5 (`mvp-else-paths-v5`) | Branch model equivalence | `formal_v5_branch_selection_matches_reference_model` | `scripts/run-formal.ps1` | no | todo (pending latest run update) | `docs/evidence/formal/latest_run.md` |
| FO-V5-003 | v5 (`mvp-else-paths-v5`) | Single-write branch effect | `formal_v5_no_dual_branch_write_effect` | `scripts/run-formal.ps1` | no | todo (pending latest run update) | `docs/evidence/formal/latest_run.md` |
| FO-V6-001 | v6 (`mvp-while-loop-v6`) | Pre-condition loop model | `formal_v6_do_while_matches_reference_model` | `scripts/run-formal.ps1` | no | todo (pending latest run update) | `docs/evidence/formal/latest_run.md` |
| FO-V6-002 | v6 (`mvp-while-loop-v6`) | Post-condition loop model | `formal_v6_post_condition_loop_semantics` | `scripts/run-formal.ps1` | no | todo (pending latest run update) | `docs/evidence/formal/latest_run.md` |
| FO-V6-003 | v6 (`mvp-while-loop-v6`) | `Exit Do` short-circuit behavior | `formal_v6_exit_do_short_circuits_iteration` | `scripts/run-formal.ps1` | no | todo (pending latest run update) | `docs/evidence/formal/latest_run.md` |

## Policy (current ladder run)
- Formal runs are required in-cycle for relevant changes.
- Formal failures are non-blocking during current ladder stage.
- Moderate in-cycle fix effort is expected; unresolved items move to the extended todo list.
