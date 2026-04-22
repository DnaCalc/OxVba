# UDT Layout Boundary Status

Date: 2026-04-22
Workset: `WORKSET_2026-04-20_VALUE_MODEL_MIGRATION_COMPARISON_AND_PERF_PLAN.md`
Bead: `bd-t8rr.7.5` / `vmm-g4`

## Purpose

Record the current migrated truth for UDT and layout-sensitive behavior so the
ABI/layout matrix bead can classify only the remaining boundary-sensitive scope.

## Current Implemented Truth

The current migrated implementation supports the bounded non-boundary UDT subset
already claimed by the baseline docs, and that subset remains intact after the
value-model migration work completed so far.

Implemented and currently exercised:

1. UDT declaration parsing and execution-path tolerance.
2. Flattened field-alias access and assignment for declared UDT values.
3. Deterministic whole-UDT copy lowering when source and target share the same
   declared UDT identity.
4. Nested UDT field expansion through compiler-side flattening.
5. Cross-type rejection for same-shape/same-field-name UDTs that are not the
   same declared type.

Code truth surfaces:

- `crates/oxvba-compiler/src/resolve.rs`
  - parses UDT blocks
  - expands nested UDT fields through `expand_nested_udt_fields(...)`
  - emits `BoundStmt::UdtAssign` only when assignment source/target share the
    same declared UDT identity
- `crates/oxvba-host/src/engine.rs`
  - contains the direct host regression lanes for whole-UDT copy, nested UDT
    field access, and cross-type rejection

This is still not a claim that runtime UDT values are laid out as native VBA or
native C structs in memory.

## Oracle and Spec Position

The current authority split remains:

1. `udt_declaration_basic.bas` is directly oracle-backed and matches Excel.
2. `udt_field_access_basic.bas`, `udt_whole_assignment_copy.bas`, and
   `udt_whole_assignment_overwrite.bas` remain closed in the conformance topic
   register with caveat because the fixtures use OxVba-extended syntax that the
   Excel oracle lane rejects (`CCT-019`).
3. `docs/evidence/SPEC_CHECKLIST.md` is still correct: the implemented claim is
   the non-boundary deterministic UDT/runtime subset, not broad boundary parity.

So the migration truth is:

- the bounded compiler/runtime UDT subset is preserved
- broader native-layout parity is still not claimed

## Explicitly Bounded / Deferred

The following remain outside the closed truth for this bead:

1. broad native-ABI struct overlay parity
2. general UDT-byref native `Declare` marshaling
3. unconstrained native field packing/alignment parity
4. any claim that internal UDT storage is already identical to Excel/VBA native
   memory layout at all boundaries

Those still belong to the explicit boundary contract and ambiguity registers:

- `docs/spec/HAL_DECLARE_ABI_SPEC_V1.md`
- `docs/evidence/hal/HAL_DECLARE_MARSHAL_AMBIGUITIES_2026-03-02.md`

## Checks Run

Focused checks run on 2026-04-22:

1. `cargo test -p oxvba-host formal_v152_udt_whole_assignment_copies_fields -- --test-threads=1`
2. `cargo test -p oxvba-host formal_v160_string_udt_coercion_corpus_fixtures_execute -- --test-threads=1`
3. `cargo test -p oxvba-host nested_udt_field_access_integration -- --test-threads=1`
4. `cargo test -p oxvba-host nested_udt_cross_type_rejection -- --test-threads=1`
5. `pwsh -File scripts/run-conformance.ps1 -Backend vm -IncludePattern udt_declaration_basic.bas`
6. `pwsh -File scripts/run-conformance.ps1 -Backend vm -IncludePattern udt_field_access_basic.bas`
7. `pwsh -File scripts/run-conformance.ps1 -Backend vm -IncludePattern udt_whole_assignment_copy.bas`
8. `pwsh -File scripts/run-conformance.ps1 -Backend vm -IncludePattern udt_whole_assignment_overwrite.bas`
9. `pwsh -File scripts/run-conformance.ps1 -Backend jit -IncludePattern udt_declaration_basic.bas`
10. `pwsh -File scripts/run-conformance.ps1 -Backend jit -IncludePattern udt_field_access_basic.bas`
11. `pwsh -File scripts/run-conformance.ps1 -Backend jit -IncludePattern udt_whole_assignment_copy.bas`
12. `pwsh -File scripts/run-conformance.ps1 -Backend jit -IncludePattern udt_whole_assignment_overwrite.bas`

Observed result:

- all focused host regression lanes passed
- all four public UDT conformance fixtures passed in both VM and JIT

## Migration Implication

`vmm-g5` should treat the UDT/layout matrix as:

1. preserve the bounded non-boundary UDT subset already implemented
2. validate ABI-sensitive pointer/native rows against the new carrier model
3. classify any remaining native-layout gaps as explicit boundary-scope items,
   not silent regressions or accidental closure claims
