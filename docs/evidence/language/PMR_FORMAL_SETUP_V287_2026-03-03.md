# PMR Formal Setup (v287) - 2026-03-03

Status: `completed`

## Goal
- Prepare execution to fully close `PMR-FUP-001` (module-aware bind/IR lowering).
- Set up `PMR-FUP-003` formal lanes (PMR + Declare invariants).
- Keep `PMR-FUP-002` deferred to COM stabilization phase.

## Implemented

1. Workset published:
- `docs/worksets/WORKSET_2026-03-03_PMR_MODULE_AWARE_BIND_AND_FORMAL_SETUP_V287.md`

2. Follow-up queue synchronized:
- `docs/worksets/WORKSET_2026-03-03_PMR_FOLLOWUP_QUEUE_FROM_OBSERVATIONS.md`
  - next block now prioritizes `PMR-FUP-001` + `PMR-FUP-003`
  - `PMR-FUP-002` explicitly deferred

3. PMR Kani harness scaffolding:
- `crates/oxvba-host/src/project.rs`
  - `pmr_typelib_resolution_transitions_typelib_refs_out_of_unbound`
  - `pmr_active_resolution_prefers_local_symbol_before_reference_symbol`

4. Declare/HAL Kani harness scaffolding:
- `crates/oxvba-hal/src/traits.rs`
  - `dynlink_contract_accepts_canonical_non_ordinal_descriptor`
  - `dynlink_contract_rejects_mismatched_selection_policy`

5. Formal registry wiring:
- `docs/evidence/formal/obligations.csv`
  - `FO-V287-001..003` added
- `docs/evidence/formal/DEFERRED_GATES.md`
  - `DG-V287-001` added as `dg-not-started` (remote lane)
- `docs/evidence/formal/EXTENDED_TODO.md`
  - `FTODO-V287-001` added

## Validation
- `cargo fmt --all` -> pass
- `cargo test -p oxvba-host project::tests:: -- --nocapture` -> pass
- `cargo test -p oxvba-hal -- --nocapture` -> pass
- `./scripts/meta-check.ps1 -Fast` -> pass

## Deferred by policy
- `PMR-FUP-002` remains deferred:
  - typelib/importlib HAL-backed resolver parity,
  - COM/oracle foldback (`CCT-043`, `ODG-041`).
