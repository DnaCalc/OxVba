# Formal Obligation Manifest

This manifest tracks profile-scoped formal obligations.

## Status Legend
- `pass`: obligation currently passing.
- `todo`: obligation currently failing or not executable; non-blocking at this stage.
- `skipped`: tooling unavailable or intentionally skipped.

## Active Obligations

| Obligation ID | Profile | Area | Harness/Proof | Runner | Blocking | Current Status | Status Source |
|---|---|---|---|---|---|---|---|
| FO-V2-001 | v2 (`mvp-controlflow-v2`) | VM control-flow safety | `pc_progression_is_safe_for_valid_jump_target` | `scripts/run-formal.ps1` | no | todo (tooling unavailable) | `docs/evidence/formal/latest_run.md` |
| FO-V2-002 | v2 (`mvp-controlflow-v2`) | Compiler temp-slot safety | `temp_slots_do_not_overlap_declared_slots` | `scripts/run-formal.ps1` | no | todo (tooling unavailable) | `docs/evidence/formal/latest_run.md` |

## Policy (current ladder run)
- Formal runs are required in-cycle for relevant changes.
- Formal failures are non-blocking during current ladder stage.
- Moderate in-cycle fix effort is expected; unresolved items move to the extended todo list.
