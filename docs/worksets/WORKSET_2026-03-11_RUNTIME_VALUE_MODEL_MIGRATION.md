# Workset: Runtime Value Model Migration

Date: 2026-03-11  
Status: active  
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
1. [register_file.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-vm\src\register_file.rs) has now been migrated to `Vec<RuntimeValue>`, but the surrounding execution substrate is still only partially widened.
2. [traits.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\traits.rs) still defines `ValueToken = i32`.
3. VM read/write helpers and snapshots now have additive value-path support, but much of the interpreter/runtime boundary still assumes the legacy integer lane.
4. Host execution helpers now expose VM-backed value snapshots, but many public/test observation paths still assume `Vec<i32>` and the Cranelift-supported subset still exposes only integer-slot semantics.
5. New semantic COM carrier/protocol slices can exist at the boundary, but cannot become the runtime’s authoritative model while these seams stay partially token-only.

Consequences:
1. object values remain awkward or impossible to carry honestly through runtime seams,
2. strings/BSTR and real SAFEARRAY payloads cannot close on the right boundary,
3. callback ingress and host-bridge object/value semantics keep narrowing too early,
4. COM cleanup risks stalling unless the runtime substrate evolves.

## 2.1 Execution status

Completed slices:
1. `RuntimeValue` now exists in [runtime_value.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-runtime\src\runtime_value.rs) as the first canonical runtime value type.
2. VM register storage now persists `RuntimeValue` rather than raw `i32`.
3. VM snapshot APIs now expose both legacy `snapshot_slots(...)` and semantic `snapshot_values(...)`.
4. Host VM execution now exposes additive value-snapshot APIs, and the legacy snapshot wrappers now project from the semantic execution lane instead of maintaining a separate token-only path.
5. COM callback ingress now hands semantic runtime values into VM procedure dispatch instead of re-narrowing through the legacy token lane.
6. JIT-backed value snapshots now preserve full `RuntimeValue` shape on VM fallback and project the supported Cranelift subset into `RuntimeValue` for host/public compatibility.
7. `CreateObject` results now enter the runtime as `RuntimeValue::ObjectHandle(...)`.
8. HAL now exposes additive semantic-return helper methods and VM host-return paths route through them.
9. The first input-side semantic HAL wrapper slice now exists for active VM host intrinsics:
   - `MsgBox` / `InputBox`,
   - `FreeFile`,
   - `Shell` / `Environ` / `Dir`,
   - `CreateObject`,
   - COM event subscription/callback helper intrinsics,
   - dynamic-link invoke wrappers.
10. VM host intrinsic execution for those lanes now reads `RuntimeValue` directly and no longer forces `read_slot(...)` legacy narrowing before the HAL boundary.
11. The shared COM carrier now preserves runtime strings and `ObjectHandle(...)` semantics on outbound native COM argument marshalling instead of degrading them to plain integer tokens before the Windows `VARIANT` boundary.
12. The supported native COM runtime-value return seam now binds `VT_DISPATCH` results back into adapter-owned object handles instead of forcing them through the legacy scalar return lane.
13. `FileSystemHal::{open,close,seek,eof,lof,free_file}` now use direct semantic `RuntimeValue` contracts instead of token-first methods plus `*_value` wrappers.
14. VM `WithEvents` binding storage now preserves semantic `RuntimeValue` payloads instead of flattening bound source/object values to raw integers, and the corresponding intrinsics now read/write semantic values directly.
15. VM `DispatchInvoke` now reads the object slot from semantic runtime state and preserves object handles before constructing the COM request.
16. `EventPumpHal::do_events()` and `TimeLocaleHal::{date_serial_now,time_serial_now,timer_ticks}` now return semantic `RuntimeValue` directly, and VM host intrinsics consume those semantic results without an intermediate token wrapper lane.
17. `UiInteractionHal::{msg_box,input_box}` and `ProcessEnvHal::{shell,environ,dir}` now also use direct semantic `RuntimeValue` contracts, and the VM/conformance/test surfaces consume those domains without token-first wrapper methods.
18. `DynamicLinkHal::{invoke_symbol,invoke_descriptor}` and `DiagnosticsHal::emit` now also use direct semantic `RuntimeValue` contracts on the VM/conformance-facing path instead of token-first wrapper methods.
19. `ComHal::{subscribe_event,unsubscribe_event,event_callback_subscription,event_callback_arity,event_callback_arg,release_event_callback}` now also use direct semantic `RuntimeValue` contracts on the VM/conformance-facing path instead of token-first wrapper methods.

Remaining blocker seam:
1. HAL `ValueToken = i32` still anchors many remaining seams,
2. the remaining holdouts are now concentrated in:
   - `ComHal::{create_object,create_object_value,release_object,describe_object,dispatch_invoke_v2,dispatch_invoke_runtime_value_v2}`,
   - `DynamicLinkHal::{bind_descriptor,prepare_invoke,invoke_bound}`,
   - engine/public callers that still observe COM object identity through raw integer tokens,
3. many public observation APIs still expose the legacy integer lane as the primary compatibility surface,
4. JIT internals and parity harnesses still observe only the integer slot lane for Cranelift-supported subsets,
5. many tests still assume integer snapshots as the primary public contract.

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

Progress:
1. `RegisterFile` migration is complete for the VM storage layer.
2. additive VM/host value snapshot surfaces are in place.
3. COM callback procedure ingress now uses semantic runtime values.
4. JIT-backed value snapshots now participate in the semantic observation surface through compatibility projection.
5. the runtime now distinguishes COM object identity semantically on the `CreateObject` path.
6. HAL semantic-return helper wrappers are now in place and used by the VM for host-return paths.
7. this phase remains open until the broader runtime-facing APIs stop depending on the legacy slot lane as their primary contract.

### Phase C. Boundary migration

Deliverables:
1. redesign `ValueToken` and affected HAL seams,
2. migrate host runtime execution helpers,
3. migrate callback ingress and dynamic-object carrier handoff,
4. maintain non-Windows deterministic unsupported behavior.

Acceptance:
1. runtime-facing external value carriers and dynamic-object protocol can traverse the core seams without early narrowing.

Current blocker:
1. the next required migration seam is the actual HAL `ValueToken` contract, the remaining token-bound HAL domains beyond the newly widened host intrinsic lanes, and the remaining legacy public observation contracts.

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
