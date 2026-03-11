# Workset: Repurpose `oxvba-com` and Extract COM Out of `oxvba-hal`

Date: 2026-03-09  
Status: planned  
Scope: redefine `oxvba-com` as the Windows-first bidirectional COM bridge for OxVba, move COM-specific state and behavior toward that crate, and shrink `oxvba-hal` back toward profile/policy/bootstrap concerns rather than serving as the long-term COM implementation home.

## 1. Decision summary

Decision lock:
1. `oxvba-com` is retained and repurposed.
2. The repurposed crate is not a placeholder or tiny shared-types crate.
3. It becomes the authoritative bridge that:
   - makes external COM objects/interfaces look like OxVba/VBA object surfaces to the runtime,
   - exposes OxVba/VBA objects outward as COM-compatible objects/interfaces,
   - owns COM-specific transport, state, lifecycle, and mapping concerns.
4. `oxvba-hal` remains the host capability/profile layer and bootstrap seam, but it is no longer the intended long-term home for rich COM activation/dispatch/event semantics.

Design posture:
1. Windows-first, COM-first.
2. Treat COM as a transport/integration domain, not as generic portable HAL behavior.
3. Keep portability discipline at the HAL boundary, but do not force COM shape into generic host-capability traits where that produces distortion.
4. Keep OxVba semantic values canonical across compiler, VM, host, and runtime surfaces; `oxvba-com` translates those values to and from COM wire formats rather than making COM wire types the core value model.

## 2. Problem statement

Current state:
1. Real COM client/event/type-library behavior lives largely in `crates/oxvba-hal/src/adapters/standard.rs`.
2. `oxvba-com` remains scaffolding from an earlier plan and does not carry the actual COM bridge burden.
3. This has pushed a large Windows-specific domain into the HAL surface:
   - COM activation
   - dispatch invoke
   - event subscription/callback interrogation
   - type-library metadata loading
4. The result is awkward:
   - COM-specific concepts leak through HAL traits,
   - `standard.rs` is oversized and difficult to audit,
   - the review correctly perceives COM as “out of place” inside the general host-capability layer.

Required correction:
1. formalize `oxvba-com` as the COM boundary crate,
2. stage extraction of COM implementation/state from `oxvba-hal`,
3. retain enough temporary compatibility so current lanes can continue closing while the boundary is refactored deliberately.

## 3. Target architecture

### 3.1 `oxvba-com` owns

`oxvba-com` becomes responsible for:
1. Boundary translation between OxVba semantic values and COM wire formats:
   - `VARIANT`,
   - `BSTR`,
   - `SAFEARRAY`,
   - `IDispatch`/interface-pointer carriers,
   - connection-point callback payload packing/unpacking.
2. COM-facing object/value model at the transport boundary:
   - object handles/tokens,
   - invoke descriptors,
   - callback payloads,
   - typed event subscription identities.
3. COM client integration on Windows:
   - activation/binding,
   - invoke (`method`, `property get`, `property put`, `property putref`),
   - argument packing/unpacking,
   - HRESULT/EXCEPINFO mapping,
   - connection-point and source-interface event delivery.
4. COM metadata and type information support:
   - typelib resolution/loading,
   - member metadata,
   - dispatch ids,
   - default-member markers,
   - event metadata.
5. COM state/lifecycle:
   - binding tables,
   - subscription tables,
   - callback queues/payload ownership,
   - release/teardown semantics,
   - deterministic cleanup helpers.
6. COM server/export bridge:
   - exposing OxVba objects outward as COM-compatible automation objects,
   - later Office-compatible server-side behavior where in scope.
7. COM-specific test fixtures:
   - controlled in-process test servers,
   - helper factories,
   - COM interop harness utilities.

### 3.2 `oxvba-hal` owns

`oxvba-hal` remains responsible for:
1. runtime profile selection,
2. policy gating and host operating envelope,
3. non-COM host capability services:
   - UI interaction,
   - event pumping/yield semantics as host/runtime concern,
   - filesystem,
   - process environment,
   - time/locale,
   - dynamic linking,
   - diagnostics.
4. bootstrap seams that let the host/runtime acquire optional integration services.

Target reduction:
1. HAL should not remain the detailed home of COM invoke/event/type-library protocols.
2. If a temporary HAL-facing COM seam remains during migration, it should narrow toward a bootstrap/delegation role rather than continuing to expand as the authoritative COM contract.

### 3.3 `oxvba-host` and runtime own

1. VBA semantics and runtime behavior remain outside `oxvba-com`.
2. The engine/runtime should consume a transport-neutral or OxVba-semantic object/event/value model where feasible.
3. `oxvba-com` is the adapter that maps between that engine-facing model and Windows COM.
4. Layout-level compatibility with COM types may be used internally, but the semantic ownership of values remains on the OxVba side.

## 4. Boundary rules

### 4.1 What should move from `oxvba-hal` to `oxvba-com`

Candidate extraction set:
1. COM binding/state structs and lifecycle maps.
2. Callback payload management and polling protocol.
3. Typelib metadata loading and COM member/event metadata builders.
4. Windows COM raw interop helpers and vtable ownership logic.
5. Controlled COM test dispatch/server implementation.
6. Event sink/source-interface implementations.
7. COM wire-format translation helpers and coercion policy.

### 4.2 What should stay out of `oxvba-com`

1. Generic UI/file/env/time/dynlink host services.
2. General runtime profile selection/policy presets.
3. Core VBA binder/runtime semantics that are not COM transport-specific.
4. Generic project/tooling responsibilities.

### 4.3 Temporary compatibility rule

During migration:
1. current behavior may continue to be reachable through HAL-backed call sites,
2. but new design work should target `oxvba-com` as the long-term owner,
3. HAL additions should avoid deepening the generic COM trait surface unless required as an interim bridge,
4. bytecode/VM/runtime APIs should not adopt raw COM wire structs as their canonical value representation,
5. and any temporary adapter-local conversion should be treated as debt to be removed once the OxVba-side carrier is in place.

## 5. Execution plan

### Phase A. Boundary lock and shared transport types

1. Define the canonical OxVba-side external value carrier used across compiler, VM, host, and runtime boundaries:
   - scalar values,
   - null/error states,
   - object identity,
   - string payload intent,
   - array payload intent.
2. Create the COM-facing transport and translation types in `oxvba-com`:
   - invoke request/response shape,
   - callback payload,
   - typed tokens/handles,
   - error mapping helpers,
   - semantic-value <-> COM-wire translators.
3. Update planning/spec docs so this crate role is explicit.
4. Keep implementation behavior unchanged where possible.

Primary outcome:
1. new COM work stops inventing permanent API shape inside HAL or pushing raw COM wire types into the VM/compiler boundary.

### Phase B. State and metadata extraction

1. Move COM state containers and metadata-loading logic from `standard.rs` into `oxvba-com`.
2. Move COM wire-format translation and coercion policy behind the `oxvba-com` boundary.
3. Keep Windows-specific execution paths functional through delegation.
4. Establish a crate-local module structure that separates:
   - client bridge,
   - metadata,
   - events,
   - server/export,
   - test fixtures,
   - raw ffi.

Primary outcome:
1. `standard.rs` shrinks materially even before final trait cleanup.

### Phase C. Windows COM client bridge extraction

1. Move activation/invoke/event subscription/polling logic into `oxvba-com`.
2. Make `oxvba-hal` delegate or bootstrap rather than implement the details directly.
3. Replace lossy token-only COM argument/result transport with the canonical OxVba-side carrier.
4. Close `release_object`, callback payload, and invoke-v2 gaps in the extracted surface.

Primary outcome:
1. the active COM parity work lands in the correct crate instead of reinforcing the wrong boundary.

### Phase D. Server/export and fixture extraction

1. Move the controlled COM fixture out of `standard.rs`.
2. Establish the outward projection model for exposing OxVba objects as COM.
3. Keep Office-style server expectations and external fixture behavior aligned with the compliance ladder.

Primary outcome:
1. bidirectional COM support becomes explicit and testable in one home.

### Phase E. HAL contraction and cleanup

1. Remove or narrow COM-heavy HAL traits/methods once delegation is complete.
2. Reconcile `HostServices` and any bootstrap seams to reflect the new boundary.
3. Remove stale scaffold from the old `oxvba-com` layout and drop dead dependencies if no longer needed.
4. Verify that VM/compiler/host boundaries remain on OxVba semantic values rather than COM wire representations.

Primary outcome:
1. HAL becomes more coherent as a host/profile layer.

## 6. Relation to current review triage

This decision resolves and re-frames several review items:
1. resolves `F-01` from `docs/REVIEW_20260309_FOLLOWUP.md`,
2. converts the “what is `oxvba-com` for?” question into an explicit design lock,
3. provides the target home for:
   - COM invoke v2 continuation,
   - callback payload cleanup,
   - `release_object`,
   - `standard.rs` modularization,
   - controlled COM fixture extraction.

It does not mean:
1. immediate full extraction before current COM correctness gaps are addressed,
2. preserving any current `oxvba-com` scaffolding as-is,
3. pushing generic host/profile concerns into `oxvba-com`.

## 7. Ladder mapping

Primary ladder support:
1. `v506-v526` COM invoke, marshaling, metadata, and integration closure
2. `v527-v533` COM server model closure
3. `v536-v539` COM event parity and integrated event gate

Secondary ladder support:
1. `v540-v544` host model integration, where clearer boundary lines will matter
2. `v553-v556` formal/safety obligations once unsafe/client/server ownership boundaries are cleaner

## 8. Risks and controls

Risks:
1. extraction churn could destabilize active COM lanes if done too early,
2. dual boundary period could create temporary duplication between HAL and `oxvba-com`,
3. server/export ambitions could over-expand scope before client parity is fully locked.

Controls:
1. keep the extraction staged and evidence-backed,
2. prioritize client invoke/event correctness before broad cleanup,
3. require each extraction step to preserve existing conformance lanes,
4. treat bidirectional server/export support as explicit scope, not hidden future intent.

## 9. Verification

Required checks for the repurpose/extraction program:
1. existing registered COM event evidence remains green,
2. controlled COM client lanes remain green through each migration step,
3. invoke-v2/property get/put/putref tests remain green,
4. server/export fixture tests remain green once extracted,
5. `standard.rs` responsibility surface measurably shrinks,
6. architecture/spec docs reflect the new crate boundary truthfully.

## 10. Immediate next actions

1. Record this decision in review triage and active workset docs.
2. Treat `oxvba-com` repurpose as the architectural target for the ongoing COM continuation work.
3. In the next implementation pass, define the canonical OxVba-side value carrier before widening more COM marshalling behavior.
4. Prefer moving new COM transport/translation work into `oxvba-com` instead of deepening HAL-owned COM APIs.
5. Treat `docs/worksets/WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md` as the focused client-side completion plan for the remaining late-bound `IDispatch` surface.
