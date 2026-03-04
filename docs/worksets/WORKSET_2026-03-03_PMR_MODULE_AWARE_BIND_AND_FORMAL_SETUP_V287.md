# WORKSET_2026-03-03_PMR_MODULE_AWARE_BIND_AND_FORMAL_SETUP_V287

## Scope
- Primary: close `PMR-FUP-001` by replacing rewrite-bridge lowering with module-aware bind/IR lowering.
- Secondary: set up `PMR-FUP-003` formal lane expansion with dedicated PMR/Declare Kani obligations.
- Deferred by policy: `PMR-FUP-002` (typelib/importlib HAL-backed resolver + COM parity/oracle foldback).

## Execution Status (2026-03-03)
- `PMR-FUP-001`: advanced to module-aware bind-plan lowering as active `compile_project(...)` path, with explicit bridge fallback (`OXVBA_PMR_LOWERING=rewrite-bridge`).
- `PMR-FUP-003`: formal setup completed (`FO-V287-001..003`, `DG-V287-001`, `FTODO-V287-001`).
- `PMR-FUP-002`: still deferred by policy (no scope expansion in this workset).
- Evidence:
  - `docs/evidence/language/PMR_FORMAL_SETUP_V287_2026-03-03.md`
  - `docs/evidence/language/PMR_MODULE_AWARE_BIND_EXECUTION_V287_2026-03-03.md`

## Why this workset
- Current `compile_project(...)` path still relies on source rewriting prior to backend compile.
- The rewrite path is useful as a bridge, but it is harder to reason about than direct binder+IR contracts.
- PMR correctness claims need dedicated formal lanes tied to the new contracts.

## Out of scope
- Typelib/importlib HAL integration and COM parity (`CCT-043`, `ODG-041`) beyond explicit backlog references.
- New COM feature breadth; this workset is structural/compiler-formal hardening.

## Detailed Work List

### Phase A - Bind/IR migration plan (`PMR-FUP-001`)
1. Freeze the rewrite-bridge baseline:
- Record current invariants and edge behavior in test names and comments.
- Ensure rewrite path remains covered while migration proceeds.

2. Introduce module-aware symbol plan artifacts:
- Build an explicit call-target plan from ProjectGraph symbols (project/module/procedure owner).
- Include function-result symbol mapping as a first-class plan element.

3. Add IR-oriented PMR lowering seam:
- Translate call-target plan into backend-consumable symbol bindings without mutating source text.
- Preserve deterministic normalization and reference-order precedence.

4. Dual-path execution guard:
- Keep rewrite path behind an internal fallback switch while module-aware path is proven.
- Add parity tests that compare call-target outcomes between old and new path on shared fixtures.

5. Promote module-aware path to primary:
- Switch `compile_project(...)` to default to module-aware lowering.
- Leave rewrite fallback available only for controlled diagnostics while burn-in completes.

6. Retire rewrite bridge for active path:
- Remove old rewrite calls from main compile flow once parity coverage is green.
- Keep only minimal helper code if needed for diagnostics/foldback evidence.

7. Contract hardening after cutover:
- Add postconditions ensuring exported surface and lowered-symbol mapping remain deterministic.
- Add regression tests for ambiguity, reference precedence, qualification, and Option Private boundaries.

### Phase B - Formal lane setup (`PMR-FUP-003`)
8. Add dedicated PMR Kani harnesses:
- PMR graph/resolution invariant harnesses in host project model.
- Focus: deterministic local-vs-reference resolution and reference-state transition safety.

9. Add dedicated Declare/HAL Kani harnesses:
- Dynamic-link descriptor contract harnesses in HAL traits.
- Focus: selection-policy/ordinal consistency and deterministic contract acceptance/rejection.

10. Register formal obligations:
- Add new `FO-*` entries for the harnesses in `docs/evidence/formal/obligations.csv`.
- Keep obligations non-blocking under current formal policy.

11. Register deferred formal lane:
- Add `DG-*` row in `docs/evidence/formal/DEFERRED_GATES.md` as `dg-not-started` (remote lane).
- Add matching `FTODO-*` row in `docs/evidence/formal/EXTENDED_TODO.md`.

12. Publish evidence note:
- Add implementation/evidence note summarizing:
  - migration plan state,
  - formal lane setup state,
  - explicit defer of typelib/COM parity track.

## Exit Criteria
1. A concrete, ordered migration plan exists and is linked from PMR follow-up docs.
2. PMR/Declare Kani harnesses compile in harness mode and are registered as formal obligations.
3. Deferred formal lane register + extended todo are updated for remote execution.
4. `PMR-FUP-002` remains explicitly deferred with no accidental scope creep.

## Validation Commands
```powershell
cargo test -p oxvba-host project::tests:: -- --nocapture
cargo test -p oxvba-hal
./scripts/meta-check.ps1 -Fast
```
