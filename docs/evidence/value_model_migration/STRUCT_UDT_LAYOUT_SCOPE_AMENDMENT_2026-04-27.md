# Struct / UDT / Native Layout Scope Amendment

Date: 2026-04-27
Workset: `WORKSET_2026-04-20_VALUE_MODEL_MIGRATION_COMPARISON_AND_PERF_PLAN.md`
Bead: `bd-t8rr.7.7` / `vmm-g6`

## Decision

The current value-model migration does not deliver broad native struct-overlay
parity or unconstrained UDT-byref native ABI parity.

The scoped migration endpoint for the struct / UDT / native-layout family is
narrowed to the following delivered truth:

1. the bounded non-boundary compiler/runtime UDT subset is preserved after the
   value-model migration
2. pointer-helper ABI-sensitive cells are reconciled to the migrated `BStr`,
   `Variant`, `SafeArray`, and `ObjectRef` substrate
3. supported native declare/writeback scalar and string lanes remain green under
   the migrated value model
4. selected ABI/layout matrix rows pass on both the fixed baseline and migrated
   candidate
5. broad native struct-overlay and unconstrained UDT-byref native ABI behavior
   remain outside the current migration scope unless a later workset expands
   that scope explicitly.

This amendment is the explicit narrowing required before closure can be
attempted for this family. It must not be read as a broad native-layout parity
claim.

## Basis

The active user instruction for this run is to continue the active migration
workset to completion. The current workset text already records that broad
native UDT-byref and struct-overlay parity is separate follow-on work if scope
expands, not a required part of this migration completion gate.

Therefore the closure path for `vmm-g6` is the narrowed route: preserve and
prove the current bounded UDT/runtime and ABI-sensitive boundary subset, then
state the residual boundary explicitly.

## Evidence Inputs

The narrowing rests on the already-landed G-lane evidence:

1. [UDT_LAYOUT_BOUNDARY_STATUS_2026-04-22.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/UDT_LAYOUT_BOUNDARY_STATUS_2026-04-22.md)
   records the implemented non-boundary UDT subset and the deferred native
   layout boundaries.
2. [ABI_LAYOUT_MATRIX_2026-04-22.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/ABI_LAYOUT_MATRIX_2026-04-22.md)
   records the paired ABI/layout matrix and classifies broad native
   struct-overlay and unconstrained UDT-byref ABI parity outside the closed
   matrix.
3. [POINTER_HELPER_ABI_CELL_RECONCILIATION_2026-04-22.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/POINTER_HELPER_ABI_CELL_RECONCILIATION_2026-04-22.md)
   records the pointer-helper cells reconciled to the migrated carrier model.
4. [NATIVE_DECLARE_WRITEBACK_RECONCILIATION_2026-04-22.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/NATIVE_DECLARE_WRITEBACK_RECONCILIATION_2026-04-22.md)
   records the supported native declare/writeback subset under the migrated
   model.

## Residual Classification

| Surface | Current status | Residual disposition |
|---|---|---|
| UDT declaration, flattened field access, nested field expansion, same-type whole-value copy, cross-type rejection | implemented-subset | closed for the bounded non-boundary compiler/runtime subset |
| Pointer helper object, string, and Variant/array cells | implemented for migrated boundary cells | covered by G1/E/D evidence and matrix rows |
| Native scalar/string declare and writeback rows | implemented-subset | closed for the selected supported rows |
| Broad native struct-overlay parity | bounded | outside current migration scope; future work only if scope expands |
| Unconstrained UDT-byref native ABI parity | bounded | outside current migration scope; future work only if scope expands |

## Closure Implication

`vmm-g6` can close on the narrowed migration scope because the repo no longer
depends on silently substituting the bounded compiler/runtime UDT subset for
broad native-layout closure.

`vmm-g7` must still record the final implemented / projected / bounded
classification before the G-family can be treated as execution-clean.
