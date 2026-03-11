# Workset: Unified Dynamic Object Protocol and External Value Carrier

Date: 2026-03-11  
Status: in-progress  
Primary ladder mapping: `v497..v505`, `v506..v513`, `v524..v526`  
Secondary ladder mapping: `v534..v539`, `v540..v544`

## 1. Objective

Define and implement the shared internal runtime contract that late-bound OxVba/VBA objects and COM-backed objects will both use.

This workset closes the architectural gap between:
1. native VBA object late binding,
2. COM `IDispatch` late binding,
3. current lossy external-call value transport.

The target is:
1. one internal late-bound object protocol shaped by VBA semantics,
2. one canonical OxVba-side external value carrier for dynamic object calls and callbacks,
3. `oxvba-com` adapting COM-backed objects to that protocol rather than preserving a COM-special runtime lane.

## 2. Why this workset exists

Current problems:
1. late-bound COM still has a partially separate execution path in code shape and docs,
2. the shared invoke lane is still limited by lossy `i32` token transport,
3. property/default-member closure is harder while native and COM-backed dynamic-object semantics are not unified,
4. further adapter-local COM patches would entrench the wrong boundary.

This workset is the runtime-facing cleanup needed before broader COM parity work can close honestly.

## 3. Target architecture

### 3.1 Unified late-bound object protocol

Define one internal dynamic-object protocol with at least:
1. object handle/identity,
2. member identity or member-resolution request,
3. call kind:
   - method
   - property get
   - property let
   - property set
4. named arguments,
5. omitted arguments,
6. default-member intent,
7. release semantics,
8. event subscription/callback identities.

### 3.2 Canonical external value carrier

Define one OxVba-side carrier for dynamic object arguments/results/callback payloads covering at least:
1. scalar numeric and boolean values,
2. null/error/empty states,
3. string payloads,
4. object handles,
5. array payload intent and supported array payloads.

This carrier must remain OxVba-semantic rather than becoming a raw COM wire-format struct.

### 3.3 Adapter rule

`oxvba-com` must adapt COM-backed objects to this protocol:
1. member resolution,
2. invoke/get/let/set,
3. argument/result translation,
4. callback payload translation,
5. deterministic error/result mapping.

## 4. Scope

### In scope

1. Runtime-facing late-bound object protocol design and implementation.
2. Canonical external-call value carrier design and implementation.
3. Bytecode/VM/host plumbing needed to carry that protocol and carrier.
4. COM-backed-object adaptation to the shared protocol.
5. Callback payload transport alignment to the same carrier/protocol family.

### Out of scope

1. Full COM server/export completion.
2. Full Office oracle closure for this area.
3. Full early-bound metadata coverage by itself.
4. Every remaining `VARIANT`/SAFEARRAY parity detail before the shared protocol exists.

## 5. Deliverables

1. Protocol note or executable API definition for the unified late-bound object contract.
2. Canonical OxVba-side value carrier type(s).
3. Compiler/bytecode representation updated so dynamic external calls are not forced through the old lossy lane.
4. VM/runtime execution path updated to use the new carrier/protocol.
5. `oxvba-com` adapter path updated to consume the new protocol instead of a COM-special lane.
6. Tests for:
   - native object dynamic calls,
   - COM-backed dynamic calls,
   - property get/let/set intent transport,
   - callback payload transport,
   - deterministic unsupported/error behavior.

## 6. Execution phases

### Phase A. Protocol lock

Deliverables:
1. define the protocol surface and operation vocabulary,
2. define the canonical value carrier,
3. define ownership boundaries between runtime, `oxvba-com`, and HAL.

Acceptance:
1. there is one explicit runtime-facing model for dynamic object calls and callback payloads.

Progress:
1. first executable semantic carrier slice is now in place in `oxvba-com` as `ComValue`,
2. shared COM request/callback structs now use that carrier instead of raw `i32` values,
3. array/null/error intent now survives to the COM boundary before any Windows-specific narrowing occurs,
4. `oxvba-com` now exposes an executable generic dynamic-object protocol API:
   - `DynamicCallRequest`
   - `DynamicMemberSelector`
   - `DynamicCallKind`
   - `DynamicEventPayload`
5. current COM request/payload types can now convert into that generic protocol shape.

### Phase B. Core transport replacement

Deliverables:
1. replace the current lossy token-only path in the relevant bytecode/VM/host seams,
2. preserve deterministic behavior for the currently supported subset while moving to the new carrier,
3. keep non-Windows deterministic unsupported behavior stable.

Acceptance:
1. supported dynamic calls are no longer fundamentally constrained by the old `i32` lane.

Progress:
1. VM `DispatchInvoke` construction now preserves recoverable semantic value shape into the shared COM request,
2. Windows adapter request/result translation now consumes the semantic carrier for the supported subset,
3. callback payload polling now returns the same carrier family.

Open remainder:
1. object identity, BSTR/string payloads, and real SAFEARRAY contents still need carrier representation,
2. wider runtime/host ingestion still narrows back into the old token lane after the COM boundary,
3. the unified late-bound object protocol is defined in code but is not yet wired through compiler/VM/host as the single runtime model,
4. the next execution blocker is now explicit:
   - `BLK-RUNTIME-VALUE-MODEL-001`
  - VM register storage and additive value snapshots have started migrating, but HAL value tokens, callback ingress, JIT/public observation surfaces, and many execution/test seams are still materially `i32` based.
5. the blocker now has a dedicated execution owner:
   - `docs/worksets/WORKSET_2026-03-11_RUNTIME_VALUE_MODEL_MIGRATION.md`

### Phase C. COM adaptation alignment

Deliverables:
1. make COM-backed objects adapt to the shared protocol in `oxvba-com`,
2. ensure property/default-member intent survives into COM adaptation,
3. align callback payload transport with the same carrier family.

Acceptance:
1. COM-backed objects are no longer a special top-level runtime path.

### Phase D. Runtime/property integration

Deliverables:
1. align the shared protocol with property/default-member intent work,
2. confirm `Set`/`Let`-sensitive operations have the right runtime contract shape,
3. update the owning blocker/worklist/spec surfaces.

Acceptance:
1. `IP-02` and `IP-03` can progress on a shared semantics model rather than divergent native-vs-COM assumptions.

## 7. Verification

Core verification:

```powershell
cargo test -p oxvba-compiler -p oxvba-vm -p oxvba-host -p oxvba-com -p oxvba-hal --quiet
cargo test -p oxvba-host --test com_client_end_to_end -- --test-threads=1 --nocapture
./scripts/check-governance.ps1
./scripts/meta-check.ps1 -Fast -NoArtifacts
```

Targeted expectations:
1. no regression in native dynamic-object behavior,
2. no regression in controlled COM client lanes,
3. callback payload tests still pass,
4. deterministic unsupported/error behavior remains stable.

## 8. Exit criteria

This workset is complete when:
1. one unified internal late-bound object protocol exists and is the authoritative runtime model,
2. one canonical OxVba-side external value carrier replaces the old lossy invoke path for the supported categories,
3. COM-backed objects adapt into that protocol through `oxvba-com`,
4. property/default-member follow-on work can use that shared protocol without introducing a separate COM exception path,
5. docs and blockers reflect the new runtime boundary truthfully.

## 9. Related documents

- `docs/spec/COM_REFERENCE_FACADE_AND_DYNAMIC_OBJECT_PROTOCOL_V1.md`
- `docs/worksets/WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`
- `docs/worksets/WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md`
- `docs/worksets/WORKSET_2026-03-11_RUNTIME_VALUE_MODEL_MIGRATION.md`
- `docs/worksets/WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md`
- `CURRENT_BLOCKERS.md`
