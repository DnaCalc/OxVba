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

- [ ] The repo has an executable host-project model, not only a design contract.
- [ ] Office-style root/global exposure rules are explicit in the supported host subset.
- [ ] Host project objects participate in the shared property/default-member model for the supported host subset.
- [ ] Host runtime session lifecycle and callback/event ingress have an implementation-backed path that matches the supported host object model.
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
- plain project references do not gain this host-root behavior; they remain on the ordinary unresolved-name / implicit-variant path in the current language mode

## Remaining checklist by closure domain

### A. Root/global exposure

- [x] Start an explicit `IP-08A` checklist and exit gate.
- [x] Prove bounded host-injected predeclared/global implicit receiver reads.
- [ ] Extend the same host-injected root/global rules to the supported write lanes where intended.
- [ ] Extend the same host-injected root/global rules to statement-context and `Call` forms where intended.
- [ ] Classify deterministic diagnostics for host-looking names that are not valid host roots in the supported subset.

### B. Host project model

- [ ] Make the host project object model executable beyond bounded implicit receiver reads.
- [ ] Prove project/object identity rules for host roots versus plain project references.
- [ ] Prove the supported host project lifecycle and session ownership behavior.

### C. Event/callback integration

- [ ] Connect host object identity to the now-executable host event ingress path where the foundation requires it.
- [ ] Prove the supported callback/event routing path against live host-backed objects rather than only synthetic project/runtime state.

### D. Handoff to `IP-08B`

- [ ] Narrow the remaining gap so `IP-08B` owns broader Office-style parity, not missing foundation semantics.
- [ ] Update blockers/worklists so the remaining host gap is described as parity breadth rather than absent host substrate.
