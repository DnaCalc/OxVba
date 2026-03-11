# Workset: Runtime Value Model Migration

Date: 2026-03-11  
Status: planned  
Primary ladder mapping: `v497..v505`, `v506..v513`  
Secondary ladder mapping: `v524..v526`, `v540..v544`

## 1. Objective

Replace the current globally `i32`-token-based execution substrate with a canonical OxVba runtime value model or indirection model that can carry the semantic value categories required by:
1. unified late-bound object calls,
2. COM/client-server interop,
3. host object/value bridging,
4. property/default-member semantics,
5. future broader marshaling work.

This workset exists because the current runtime is still structurally centered on:
1. `Vec<i32>` VM register storage,
2. `ValueToken = i32` at the HAL seam,
3. host/runtime snapshots and callback ingress built around raw `i32` slots.

That model was sufficient for the current subset, but it is now the real blocker for further honest progress.

## 2. Problem statement

Current blocker:
1. `BLK-RUNTIME-VALUE-MODEL-001`

Current structural constraints:
1. [register_file.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-vm\src\register_file.rs) stores `Vec<i32>`.
2. [traits.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\traits.rs) defines `ValueToken = i32`.
3. VM read/write helpers, snapshots, and many interpreter operations assume raw integer slots.
4. Host execution helpers and tests assume `Vec<i32>` snapshots as the public observation surface.
5. New semantic COM carrier/protocol slices can exist at the boundary, but cannot become the runtime’s authoritative model while these seams stay token-only.

Consequences:
1. object values remain awkward or impossible to carry honestly through runtime seams,
2. strings/BSTR and real SAFEARRAY payloads cannot close on the right boundary,
3. callback ingress and host-bridge object/value semantics keep narrowing too early,
4. COM cleanup risks stalling unless the runtime substrate evolves.

## 3. Target architecture

### 3.1 Canonical runtime value model

Define one canonical runtime value representation or indirection model for VM/host/runtime coordination that can represent at least:
1. empty/null/error states,
2. scalar numeric and boolean values,
3. strings,
4. arrays,
5. object references/handles,
6. future extended scalar categories as required.

This model must remain OxVba-semantic:
1. not raw COM wire types,
2. not a COM-special runtime lane,
3. not a host-specific alternate value system.

### 3.2 Compatible execution strategy

The migration may use one of two acceptable shapes:
1. replace slot storage with richer runtime values directly,
2. keep slot-like storage but make it an indirection/index into a richer value arena.

The design choice must satisfy:
1. deterministic behavior,
2. tractable JIT/VM parity maintenance,
3. manageable host/HAL boundary migration,
4. compatibility with the unified late-bound object protocol and external value carrier.

### 3.3 Boundary rule

After migration:
1. HAL must no longer be semantically limited by `ValueToken = i32`,
2. VM/host/runtime seams must no longer force object/string/array semantics through raw integer narrowing,
3. `oxvba-com` remains responsible for COM wire translation,
4. the core runtime remains responsible for semantic values.

## 4. Scope

### In scope

1. Runtime value model design lock.
2. VM register storage migration or indirection-layer migration.
3. VM read/write/snapshot API updates.
4. HAL `ValueToken` seam redesign.
5. Host/runtime callback ingress and snapshot surface redesign.
6. JIT/VM/test migration required by the new value model.
7. Documentation and blocker/worklist updates.

### Out of scope

1. Full COM parity closure by itself.
2. Full property/default-member closure by itself.
3. Full host project/Office-style hosting closure by itself.
4. Full oracle/formal closure by itself.

## 5. Deliverables

1. Runtime value-model decision note embedded in code/docs.
2. New canonical runtime value type or value-indirection substrate.
3. VM register file and read/write helpers migrated.
4. HAL boundary updated away from raw `i32`-only semantics.
5. Host/runtime snapshot and callback-ingress surfaces updated.
6. A compatibility strategy for tests/JIT/fixtures.
7. Updated blockers/worksets/spec cross-links.

## 6. Execution phases

### Phase A. Value-model decision lock

Deliverables:
1. choose direct rich-slot model vs. arena/handle indirection model,
2. define the canonical runtime value categories,
3. define the migration compatibility rules for VM, HAL, host, and tests.

Acceptance:
1. one explicit runtime value-model choice is documented and adopted as the migration target.

### Phase B. Core VM substrate migration

Deliverables:
1. migrate `RegisterFile`,
2. migrate `read_slot` / `write_slot`,
3. migrate snapshot surfaces or add compatibility wrappers,
4. keep deterministic subset behavior stable.

Acceptance:
1. VM execution is no longer intrinsically limited to raw `i32` values.

### Phase C. Boundary migration

Deliverables:
1. redesign `ValueToken` and affected HAL seams,
2. migrate host runtime execution helpers,
3. migrate callback ingress and dynamic-object carrier handoff,
4. maintain non-Windows deterministic unsupported behavior.

Acceptance:
1. runtime-facing external value carriers and dynamic-object protocol can traverse the core seams without early narrowing.

### Phase D. Integration follow-through

Deliverables:
1. reconnect the unified dynamic-object protocol to the migrated runtime seams,
2. reconnect COM/host bridge/value carrier work on the new substrate,
3. update tests, docs, blockers, and worklists.

Acceptance:
1. `BLK-RUNTIME-VALUE-MODEL-001` is resolved,
2. `WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md` can continue beyond the current boundary slices.

## 7. Verification

Core verification:

```powershell
cargo test -p oxvba-vm -p oxvba-host -p oxvba-hal -p oxvba-com --quiet
./scripts/check-governance.ps1
./scripts/meta-check.ps1 -Fast -NoArtifacts
```

Targeted expectations:
1. VM/JIT parity remains valid for unaffected lanes,
2. snapshot-based test surfaces remain deterministic,
3. COM callback and invoke lanes do not regress,
4. host/runtime error routing remains stable.

## 8. Exit criteria

This workset is complete when:
1. the runtime no longer fundamentally depends on `Vec<i32>` / `ValueToken = i32` as its sole semantic model,
2. the canonical OxVba runtime value representation is the authoritative execution substrate,
3. unified dynamic-object protocol and external value carrier work can proceed without structural token-lane blockers,
4. docs and blockers truthfully reflect the migrated runtime boundary.

## 9. Related documents

- [WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md)
- [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
- [WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md)
- [WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md)
- [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
