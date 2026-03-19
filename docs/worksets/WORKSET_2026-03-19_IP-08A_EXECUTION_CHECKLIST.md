# WORKSET_2026-03-19_IP-08A_EXECUTION_CHECKLIST

## Purpose

Turn `IP-08A` from a design-only phase into an explicit execution checklist for the host-project / Office-style hosting foundation.

`IP-08` remains `in-progress` until host-project semantics are executable enough that the repo has a real host/root/global substrate rather than only a proposal and partial dependency slices.

## Governing sources

Primary contract sources:
- [OPERATIONS.md](C:\Work\DnaCalc\OxVba\OPERATIONS.md)
- [MACH1000_PLAN.md](C:\Work\DnaCalc\OxVba\MACH1000_PLAN.md)
- [HOSTING_PROJECT_TOOLING_PROPOSAL.md](C:\Work\DnaCalc\OxVba\docs\spec\HOSTING_PROJECT_TOOLING_PROPOSAL.md)
- [WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md)
- [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
- [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)

Binding doctrine pulled from those sources:
- `IP-08A` is the host-foundation phase, not host parity closure.
- Partial host-root behavior stays `in-progress`; do not describe bounded host-injected slices as full Office-style hosting.
- Host-project behavior must consume the settled property/object/event model and must not hide lower-layer semantic gaps.

## Exit gate

`IP-08A` is complete only when all of the following are true:

- [x] The repo has an executable host-project model, not only a design contract.
- [x] Office-style root/global exposure rules are explicit in the supported host subset.
- [x] Host project objects participate in the shared property/default-member model for the supported host subset.
- [x] Host runtime session lifecycle and callback/event ingress have an implementation-backed path that matches the supported host object model.
- [ ] Object identity boundaries between native host objects, referenced projects, and COM-backed host objects are explicit for the supported host subset.
- [ ] Remaining `IP-08` work is narrowed honestly to broader host parity closure (`IP-08B`), not missing foundation behavior.

## Lane matrix

Each lane must be classified as exactly one of:
- `proved-exec`
- `proved-diagnostic`
- `implemented-unproved`
- `missing-semantics`
- `missing-diagnostic`
- `oracle-needed`

Each lane is identified by these axes:

1. Host receiver source
- active project class module
- host-injected referenced project class module
- plain referenced project class module
- COM-backed host object

2. Exposure mode
- `VB_PredeclaredId`
- `VB_GlobalNamespace`
- no implicit exposure

3. Syntax shape
- named property/default-member read
- property/default-member write
- statement-context call
- explicit `Call`

4. Runtime path
- compile-time rewrite only
- VM/JIT host execution
- host event/callback ingress

## Current proved floor

Already evidenced in the repo today:

- explicit host-event ingress now dispatches into live runtime sessions through compiler-generated guard wrappers for the current zero/one-argument subset
- host-injected referenced class modules marked `VB_PredeclaredId` or `VB_GlobalNamespace` now participate in implicit receiver lowering for bounded property/default-member read lanes
- the proved host-injected read lanes currently cover:
  - named property-get reads such as `Application.Value`
  - authoritative default-member reads such as `Application`
- the proved host-injected read floor now also covers class-procedure comparison expressions on named property-get lanes such as `If Application.Value = 4 Then` across both exposure modes, so the host-root receiver is lowered as a read instead of being misclassified as an assignment LHS
- the proved host-injected write lanes currently cover:
  - `VB_PredeclaredId` named `Property Let` writes such as `Application.Value = 9`
  - `VB_PredeclaredId` authoritative default-member `Property Let` writes such as `Application = 9`
  - `VB_GlobalNamespace` named `Property Let` writes such as `Application.Value = 9`
  - `VB_GlobalNamespace` authoritative default-member `Property Let` writes such as `Application = 9`
- the proved host-injected explicit-`Call` lanes currently cover:
  - `VB_PredeclaredId` named property-get `Call` forms such as `Call Application.Value`
  - `VB_PredeclaredId` authoritative default-member `Call` forms such as `Call Application`
  - `VB_GlobalNamespace` named property-get `Call` forms such as `Call Application.Value`
  - `VB_GlobalNamespace` authoritative default-member `Call` forms such as `Call Application`
- the proved host-injected statement-context lanes currently cover:
  - `VB_PredeclaredId` named property-get statement forms such as `Application.Value`
  - `VB_PredeclaredId` authoritative default-member statement forms such as `Application`
  - `VB_GlobalNamespace` named property-get statement forms such as `Application.Value`
  - `VB_GlobalNamespace` authoritative default-member statement forms such as `Application`
- the proved host-injected object-return floor currently covers:
  - `VB_PredeclaredId` named object-valued `Property Get` assignment into an `Object` target such as `Set child = Application.Value`
  - `VB_GlobalNamespace` named object-valued `Property Get` assignment into an `Object` target such as `Set child = Application.Value`
- the proved host-injected child-navigation floor currently covers:
  - `VB_PredeclaredId` named object-valued host-root `Property Get` assignment into a typed child local followed by named property-get member traffic such as `Dim child As Child : Set child = Application.Value : afterValue = child.Value`
  - `VB_PredeclaredId` named object-valued host-root `Property Get` assignment into a typed child local followed by authoritative default-member traffic such as `Dim child As Child : Set child = Application.Value : afterValue = child`
  - `VB_GlobalNamespace` named object-valued host-root `Property Get` assignment into a typed child local followed by named property-get member traffic such as `Dim child As Child : Set child = Application.Value : afterValue = child.Value`
  - `VB_GlobalNamespace` named object-valued host-root `Property Get` assignment into a typed child local followed by authoritative default-member traffic such as `Dim child As Child : Set child = Application.Value : afterValue = child`
  - `VB_PredeclaredId` typed child-local parenthesized zero-arg getter traffic after host-root object return for both named and authoritative default-member forms such as `afterValue = child.Value()` and `afterValue = child()`
  - `VB_GlobalNamespace` typed child-local parenthesized zero-arg getter traffic after host-root object return for both named and authoritative default-member forms such as `afterValue = child.Value()` and `afterValue = child()`
  - `VB_PredeclaredId` typed child-local indexed getter traffic after host-root object return for both named and authoritative default-member forms such as `afterValue = child.Value(2)` and `afterValue = child(2)`
  - `VB_GlobalNamespace` typed child-local indexed getter traffic after host-root object return for both named and authoritative default-member forms such as `afterValue = child.Value(2)` and `afterValue = child(2)`
- the proved host-injected child-invoke floor currently covers:
  - `VB_PredeclaredId` typed child-local explicit `Call` traffic after host-root object return for both named and authoritative default-member zero-arg getter forms such as `Call child.Value` and `Call child`
  - `VB_PredeclaredId` typed child-local bare statement-context traffic after host-root object return for both named and authoritative default-member zero-arg getter forms such as `child.Value` and `child`
  - `VB_GlobalNamespace` typed child-local explicit `Call` traffic after host-root object return for both named and authoritative default-member zero-arg getter forms such as `Call child.Value` and `Call child`
  - `VB_GlobalNamespace` typed child-local bare statement-context traffic after host-root object return for both named and authoritative default-member zero-arg getter forms such as `child.Value` and `child`
  - `VB_PredeclaredId` typed child-local parenthesized explicit `Call` and bare statement-context traffic after host-root object return for both named and authoritative default-member zero-arg getter forms such as `Call child.Value()`, `Call child()`, `child.Value()`, and `child()`
  - `VB_GlobalNamespace` typed child-local parenthesized explicit `Call` and bare statement-context traffic after host-root object return for both named and authoritative default-member zero-arg getter forms such as `Call child.Value()`, `Call child()`, `child.Value()`, and `child()`
  - `VB_PredeclaredId` typed child-local indexed explicit `Call` and bare statement-context traffic after host-root object return for both named and authoritative default-member forms such as `Call child.Value(2)`, `Call child(2)`, `child.Value(2)`, and `child(2)`
  - `VB_GlobalNamespace` typed child-local indexed explicit `Call` and bare statement-context traffic after host-root object return for both named and authoritative default-member forms such as `Call child.Value(2)`, `Call child(2)`, `child.Value(2)`, and `child(2)`
- the proved host-injected child-write floor currently covers:
  - `VB_PredeclaredId` typed child-local named `Property Let` and authoritative default-member `Property Let` traffic after host-root object return such as `child.Value = 9` and `child = 9`
  - `VB_GlobalNamespace` typed child-local named `Property Let` and authoritative default-member `Property Let` traffic after host-root object return such as `child.Value = 9` and `child = 9`
  - `VB_PredeclaredId` typed child-local indexed `Property Let` and authoritative indexed default-member `Property Let` traffic after host-root object return such as `child.Value(2) = 11` and `child(2) = 11`
  - `VB_GlobalNamespace` typed child-local indexed `Property Let` and authoritative indexed default-member `Property Let` traffic after host-root object return such as `child.Value(2) = 11` and `child(2) = 11`
  - `VB_PredeclaredId` typed child-local named `Property Set` and authoritative default-member `Property Set` traffic after host-root object return such as `Set child.Value = x` and `Set child = x`
  - `VB_GlobalNamespace` typed child-local named `Property Set` and authoritative default-member `Property Set` traffic after host-root object return such as `Set child.Value = x` and `Set child = x`
  - `VB_PredeclaredId` typed child-local indexed `Property Set` and authoritative indexed default-member `Property Set` traffic after host-root object return such as `Set child.Value(1) = x` and `Set child(1) = x`
  - `VB_GlobalNamespace` typed child-local indexed `Property Set` and authoritative indexed default-member `Property Set` traffic after host-root object return such as `Set child.Value(1) = x` and `Set child(1) = x`
- plain project references do not gain this host-root behavior; they remain on the ordinary unresolved-name / implicit-variant path in the current language mode
- conflicting same-name plain-project class references also do not steal `HostInjected` source identity by reference order in the current `WithEvents` subset; a bound host-backed `Emitter` still routes through `HostProject.Emitter` while `PlainProject.Emitter` remains non-routing for the same bound handle
- neighboring COM-backed object handles also do not steal host-backed `WithEvents` ownership in the current subset; a bound host-backed `Emitter` still owns host event routing while a sibling `CreateObject(4)` handle remains non-routing on the same host event ingress path
- host-injected root getters may now also return bounded COM-backed objects in the current subset; a supported `Application.Value` getter can return `CreateObject(4)` and the caller can feed that returned object through `DispatchInvoke` on the shared object/value model
- the same bounded host/COM coexistence floor now also covers a first imported early-bind handoff; a supported `Application.Value` getter may return `CreateObject(4)` into `Dim obj As OxVba.TestDispatch` and the caller may execute `obj.Count()` through the imported metadata-backed member path
- the same bounded host/COM coexistence floor now also proves that a conflicting same-name plain-project `Application` reference does not steal that first imported early-bind handoff by reference order; `HostProject.Application.Value` still wins, returns `CreateObject(4)`, and feeds the bounded `obj.Count()` lane
- the same bounded host/COM coexistence floor now also covers imported property traffic on that returned COM object; a supported `Application.Value` getter may return `CreateObject(4)` into `Dim obj As OxVba.TestDispatch`, execute `obj.SetValue = 9`, and then execute `afterValue = obj.Value` on the shared object/value model
- the same bounded host/COM coexistence floor now also covers imported authoritative default-member traffic on that returned COM object; a supported `Application.Value` getter may return `CreateObject(4)` into `Dim obj As OxVba.TestDispatch` and then execute `echoValue = obj(41)` on the shared object/value model
- the same bounded host/COM coexistence floor now also covers imported object-result assignment-intent traffic on that returned COM object; a supported `Application.Value` getter may return `CreateObject(4)` into `Dim obj As OxVba.TestDispatch`, and `ReturnSelfDispatch()` / `ReturnSelfUnknown()` now preserve bounded object rebinding through explicit `Set` on `Object` targets plus implicit / explicit-`Let` assignment on `Variant` targets
- the same bounded host/COM coexistence floor now also covers imported object-valued zero-arg `PropertyGet` assignment-intent traffic on that returned COM object; a supported `Application.Value` getter may return `CreateObject(4)` into `Dim obj As OxVba.TestDispatch`, and `SelfDispatch` / `SelfUnknown` now preserve bounded object rebinding through explicit `Set` on `Object` targets plus implicit / explicit-`Let` assignment on `Variant` targets
- the same bounded host/COM coexistence floor now also proves that a conflicting same-name plain-project `Application` reference does not steal that imported object-valued `PropertyGet` handoff by reference order; `HostProject.Application.Value` still wins, returns `CreateObject(4)`, and feeds the bounded `SelfDispatch` / `SelfUnknown` assignment-intent lanes
- the same bounded host/COM coexistence floor now also covers parenthesized imported object-valued zero-arg `PropertyGet` assignment-intent traffic on that returned COM object; a supported `Application.Value` getter may return `CreateObject(4)` into `Dim obj As OxVba.TestDispatch`, and `SelfDispatch()` / `SelfUnknown()` now preserve bounded object rebinding through explicit `Set` on `Object` targets plus implicit / explicit-`Let` assignment on `Variant` targets
- host-looking names backed by `HostInjected` class modules that are present but not exposed through `VB_PredeclaredId=True` or `VB_GlobalNamespace=True` now fail deterministically across bounded read/write/`Call` forms with `PMR-E-HOST-ROOT-NOT-EXPOSED`
- the proved host runtime-session floor now also covers per-runtime host-root state isolation across live event ingress for both exposure modes in the current named-property subset, so repeated callbacks mutate only the owning runtime session and fresh sessions restart from the host baseline
- the proved host-backed callback floor now also covers live snapped source-handle routing on referenced `HostInjected` event sources, so `WithEvents` bindings remain keyed to the referenced host project/module identity and only the bound host-backed source handle routes the callback while sibling handles of the same referenced source type no-op deterministically
- broader follow-on member traffic on those returned host-root object handles is still open in the bounded subset; this checklist currently proves handle return plus named/default-member child read, parenthesized zero-arg getter syntax, indexed scalar getter/invoke/write syntax, zero-arg invoke, scalar write navigation, the current named/indexed `Property Set` navigation slice, and the current imported scalar/default-member/object-result/object-property/parenthesized-object-property traffic plus same-name plain-project precedence on host-returned COM-backed objects, not full host-foundation closure

## Remaining checklist by closure domain

### A. Root/global exposure

- [x] Start an explicit `IP-08A` checklist and exit gate.
- [x] Prove bounded host-injected predeclared/global implicit receiver reads.
- [x] Extend the same host-injected root/global rules to the supported write lanes where intended.
- [x] Extend the same host-injected root/global rules to statement-context and `Call` forms where intended.
- [x] Classify deterministic diagnostics for host-looking names that are not valid host roots in the supported subset.

### B. Host project model

- [ ] Make the host project object model executable beyond bounded implicit receiver reads, bounded object-handle return, and the current named/default-member child read/parenthesized/indexed/invoke/scalar-write navigation slice.
- [ ] Prove project/object identity rules for host roots versus plain project references.
- [x] Prove the supported host project lifecycle and session ownership behavior.

### C. Event/callback integration

- [x] Connect host object identity to the now-executable host event ingress path where the foundation requires it.
- [x] Prove the supported callback/event routing path against live host-backed objects rather than only synthetic project/runtime state.

### D. Handoff to `IP-08B`

- [ ] Narrow the remaining gap so `IP-08B` owns broader Office-style parity, not missing foundation semantics.
- [ ] Update blockers/worklists so the remaining host gap is described as parity breadth rather than absent host substrate.
