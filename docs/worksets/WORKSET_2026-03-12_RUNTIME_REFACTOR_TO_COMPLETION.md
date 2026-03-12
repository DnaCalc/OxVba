# Workset: Runtime Refactor To Completion

Date: 2026-03-12  
Status: active  
Primary ladder mapping: `v497..v505`, `v506..v513`, `v524..v526`, `v540..v544`  
Secondary ladder mapping: `v545..v560`

## 1. Objective

Carry the runtime representation refactor from the current partial semantic-value migration to full completion.

This workset is the end-to-end execution spine from:
1. the current mixed semantic/legacy runtime substrate,
2. to a fully semantic execution model,
3. to the point where COM extraction and broader parity work can continue without structural token-lane blockers.

## 2. Why this workset exists

The current migration has already established:
1. semantic register storage,
2. semantic host/value snapshots,
3. typed runtime handles,
4. typed COM boundary carriers,
5. an initial owned `RuntimeValue` <-> `Variant` bridge for the honest scalar subset.

But the runtime is not yet complete because:
1. core interpreter instruction execution still depends materially on `read_slot(...)` / `write_slot(...)`,
2. comparison/control-flow/arithmetic subsets still lean on legacy `i32` projection,
3. explicit compatibility observation still centers on `snapshot_slots(...)`,
4. the remaining COM/dynamic-object migration cannot finish honestly until those core seams are refactored.

## 3. Target state

At completion:
1. `RuntimeValue` is the authoritative execution substrate.
2. Typed semantic handles remain first-class where identity is required:
   - `ObjectHandle`
   - `BindingHandle`
3. Legacy `i32` observation survives only as an explicitly bounded compatibility projection.
4. `oxvba-runtime::Variant` is an owned implementation-aligned substrate where appropriate, not a second semantic model.
5. `oxvba-com` can continue extraction and parity work on top of the completed runtime substrate instead of transitional token seams.

## 4. Scope

### In scope

1. Core interpreter migration away from `read_slot(...)` / `write_slot(...)` as primary semantics.
2. Runtime truthiness/comparison/arithmetic helpers over semantic runtime values.
3. Compatibility observation narrowing and cleanup.
4. Further `RuntimeValue` / owned `Variant` convergence where semantically honest.
5. Host/JIT/test migration required to make semantic execution the default assumption.
6. Documentation, blocker, and worklist synchronization.

### Out of scope

1. Full COM parity closure by itself.
2. Final `oxvba-com` extraction by itself.
3. Full property/default-member closure by itself.
4. Full server/export closure by itself.

## 5. Execution plan

### Phase A. Core VM execution migration

Goal:
1. stop treating `read_slot(...)` / `write_slot(...)` as the semantic execution substrate.

Progress:
1. first semantic-execution slice completed for:
   - comparisons,
   - boolean operations,
   - `JumpIfZero`,
   - `IncSlot`.
2. those instructions now read `RuntimeValue` directly and produce semantic `RuntimeValue::Bool(...)` / `RuntimeValue::I32(...)` results while preserving legacy compatibility projection through `snapshot_slots(...)`.
3. typed comparator fastpaths were aligned to the same semantic result shape.
4. the wider interpreter loop no longer executes through the old `read_slot(...)` / `write_slot(...)` helper names; the remaining scalar/intrinsic estate now runs through explicit legacy-projection helpers over `RuntimeValue`.
5. remaining Phase A work is now concentrated in:
   - retiring or narrowing those legacy-projection helpers where full semantic execution is now possible,
   - deciding and implementing honest `CopySlot` behavior for non-legacy runtime shapes,
   - migrating the larger intrinsic families away from legacy-scalar execution where that is semantically correct.

Deliverables:
1. semantic helpers for:
   - truthiness,
   - equality and ordering,
   - arithmetic subset,
   - jump-condition evaluation.
2. interpreter instruction families migrated to semantic execution:
   - comparisons,
   - boolean operations,
   - jump/control-flow conditions,
   - scalar arithmetic/update subsets.

Acceptance:
1. the main interpreter loop no longer depends on legacy integer-slot reads for normal semantic execution.

### Phase B. Compatibility observation contraction

Goal:
1. make semantic observation the default across runtime, host, and JIT surfaces.

Deliverables:
1. audit and migrate remaining callers of:
   - `Vm::snapshot_slots(...)`,
   - engine slot snapshot wrappers,
   - JIT legacy snapshot wrappers.
2. keep legacy slot observation only where it is an explicit compatibility test or external compatibility surface.

Acceptance:
1. integer-slot observation is clearly secondary and compatibility-scoped.

### Phase C. RuntimeValue and Variant convergence

Goal:
1. make the owned runtime value model and the owned runtime `Variant` model intentionally aligned where honest.

Deliverables:
1. extend `RuntimeValue` <-> `Variant` bridging for additional honest categories only,
2. document the owned-runtime `Variant` subset vs runtime-only or boundary-only value categories,
3. keep rejected categories explicit until their ownership model is correct.

Acceptance:
1. there is no ambiguous overlap between runtime semantic values and owned `Variant` representation.

### Phase D. Dynamic object and external-call follow-through

Goal:
1. reopen the unified dynamic-object/value-carrier work on the completed runtime substrate.

Deliverables:
1. thread the unified dynamic-object protocol through the migrated runtime seams,
2. thread the semantic external-call value carrier through runtime/host/callback paths without early narrowing,
3. remove remaining runtime-side COM-special call-shape assumptions.

Acceptance:
1. unified dynamic-object/value-carrier work can continue without structural runtime blockers.

### Phase E. COM extraction follow-through

Goal:
1. allow `oxvba-com` extraction to continue on the right substrate.

Deliverables:
1. continue moving COM activation/invoke/event/type-library ownership out of transitional HAL code,
2. contract HAL toward bootstrap/profile/policy/delegation seams,
3. keep runtime and host on OxVba semantic values while `oxvba-com` owns COM translation.

Acceptance:
1. COM extraction is no longer blocked by the runtime representation itself.

### Phase F. Closure cleanup

Goal:
1. end the runtime migration cleanly.

Deliverables:
1. remove obsolete compatibility wrappers where possible,
2. update blockers/worklists/architecture/specs,
3. narrow remaining explicit compatibility tests to intentional legacy coverage only.

Acceptance:
1. `BLK-RUNTIME-VALUE-MODEL-001` is resolved,
2. [WORKSET_2026-03-11_RUNTIME_VALUE_MODEL_MIGRATION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_RUNTIME_VALUE_MODEL_MIGRATION.md) can close honestly,
3. downstream COM/runtime work can proceed without runtime-structure blocker language.

## 6. Dependencies

Primary dependencies:
1. [WORKSET_2026-03-11_RUNTIME_VALUE_MODEL_MIGRATION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_RUNTIME_VALUE_MODEL_MIGRATION.md)
2. [WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md)
3. [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)

Blocking reference:
1. [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)

## 7. Verification

Core verification:

```powershell
cargo test -p oxvba-runtime -p oxvba-vm -p oxvba-host -p oxvba-hal -p oxvba-com -p oxvba-jit --quiet
./scripts/check-governance.ps1
./scripts/meta-check.ps1 -Fast -NoArtifacts
```

Targeted expectations:
1. semantic execution becomes the default in runtime-facing tests,
2. explicit legacy slot projections remain deterministic where retained,
3. VM/JIT parity remains valid for supported subsets,
4. COM invoke/event paths do not regress while the runtime substrate is being completed.

## 8. Exit criteria

This workset is complete when:
1. the interpreter no longer fundamentally relies on legacy integer-slot reads for semantic execution,
2. semantic runtime values are the default execution and observation model,
3. compatibility slot projection is explicitly bounded and secondary,
4. the runtime/host/JIT substrate no longer blocks unified dynamic-object/value-carrier work,
5. the runtime refactor no longer appears in `CURRENT_BLOCKERS.md` as an active structural blocker.

## 9. Related documents

- [WORKSET_2026-03-11_RUNTIME_VALUE_MODEL_MIGRATION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_RUNTIME_VALUE_MODEL_MIGRATION.md)
- [WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md)
- [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
- [ARCHITECTURE.md](C:\Work\DnaCalc\OxVba\docs\ARCHITECTURE.md)
- [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
