# WORKSET_2026-03-20_COM_HOST_COMPLETION_CHECKLIST

## Purpose

Provide one ordered completion checklist for every currently non-closed COM- and host-related work area that still blocks honest parity claims in this program:

- `IP-03` Windows late-bound COM client parity
- `IP-05` Windows early-bound COM and type-library parity
- `IP-06` Windows COM server/export parity
- `IP-07` Event runtime parity
- `IP-08` Host project / Office-style hosting parity
- `IP-09` Declare/native marshaling parity

If every checklist item in this document is satisfied, then:

- the current `in-progress` rows for `IP-03`, `IP-05`, `IP-07`, `IP-08`, and `IP-09` may be moved to `closed`,
- the current `planned` row for `IP-06` may be moved to `closed`,
- only cross-program oracle/formal closure work may remain under `IP-10` / `IP-11` where ownership has already been transferred explicitly.

This document is not a replacement for the per-area worksets. It is the dependency-ordered closeout checklist that makes sure those worksets can all finish without leaving hidden cross-area debt behind.

## Governing references

Program-level governance:

- [OPERATIONS.md](C:\Work\DnaCalc\OxVba\OPERATIONS.md)
- [MACH1000_PLAN.md](C:\Work\DnaCalc\OxVba\MACH1000_PLAN.md)
- [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
- [WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md)

Area-specific canonical owners:

- `IP-03`
  - [WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md)
  - [WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md)
  - [COM_CLIENT_SERVER_SCOPE_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\COM_CLIENT_SERVER_SCOPE_V1.md)
- `IP-05`
  - [WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md)
  - [WORKSET_2026-03-18_IP-05A_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-05A_EXECUTION_CHECKLIST.md)
  - [COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md)
- `IP-06`
  - [COM_CLIENT_SERVER_SCOPE_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\COM_CLIENT_SERVER_SCOPE_V1.md)
  - [WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md)
- `IP-07`
  - [WORKSET_2026-03-08_EVENTS_PARITY_CLOSURE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-08_EVENTS_PARITY_CLOSURE.md)
- `IP-08`
  - [WORKSET_2026-03-19_IP-08A_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-19_IP-08A_EXECUTION_CHECKLIST.md)
  - [WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md)
  - [HOSTING_PROJECT_TOOLING_PROPOSAL.md](C:\Work\DnaCalc\OxVba\docs\spec\HOSTING_PROJECT_TOOLING_PROPOSAL.md)
- `IP-09`
  - [HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md)
  - [HAL_CONTRACT_CLAUSE_CATALOG_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\HAL_CONTRACT_CLAUSE_CATALOG_V1.md)

## Completion doctrine for this checklist

- A work area is not complete because a representative subset exists.
- A work area is complete only when its scoped matrix is either:
  - proved executable with compiler/host evidence, or
  - proved intentionally unsupported with deterministic diagnostics and recorded rationale.
- This checklist is complete only when every listed work area can be described as `closed` in [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) without bounded-subset caveats remaining in its own scope.

## Dependency order

Recommended execution order:

1. `IP-03`
2. `IP-05`
3. `IP-09`
4. `IP-07`
5. `IP-08`
6. `IP-06`
7. final cross-workset closure audit

Why this order:

- `IP-03` must settle the late-bound COM client/runtime truth before higher-level host/event/server work can close honestly.
- `IP-05` must settle imported metadata/member behavior before host-returned COM breadth and server-facing typelib expectations can close honestly.
- `IP-09` must settle ABI/marshaling legality before server/export parity can close and before remaining host/native edge obligations can be called complete.
- `IP-07` should close after the client/type-library/runtime substrate is stable, because full event parity crosses both COM and host ingress.
- `IP-08` should close after event ownership is clear and imported COM breadth is no longer moving beneath it.
- `IP-06` should close last, because outward COM server/export parity depends on stable client semantics, typelib semantics, event semantics, host policy, and marshaling obligations.

## Master exit gate

This checklist is complete only when all of the following are true:

- [ ] `IP-03` is marked `closed` in [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- [ ] `IP-05` is marked `closed` in [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- [ ] `IP-06` is marked `closed` in [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- [ ] `IP-07` is marked `closed` in [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- [ ] `IP-08` is marked `closed` in [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- [ ] `IP-09` is marked `closed` in [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- [ ] [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) no longer carries an open blocker owned by `IP-03`, `IP-05`, `IP-06`, `IP-07`, `IP-08`, or `IP-09`
- [ ] each area-specific workset/checklist above either records completion directly or is superseded by a closure note with no live residual in that area
- [ ] remaining oracle-only or formal-only obligations, if any, are explicitly owned by `IP-10` / `IP-11` rather than by the area itself

## Shared execution loop

Every slice taken under this checklist should follow the same loop:

1. classify the next lane or matrix fragment explicitly,
2. implement semantics or deterministic diagnostics,
3. add compiler proof,
4. add host/adapter/runtime proof,
5. update blockers/worklist/logs honestly,
6. run `cargo fmt --all`,
7. run targeted tests, then `cargo test -p oxvba-com -p oxvba-compiler -p oxvba-hal -p oxvba-host --quiet`,
8. run `./scripts/check-governance.ps1`,
9. run `./scripts/meta-check.ps1 -Fast -NoArtifacts`,
10. commit and push.

## Workset breakdown

### `IP-03` Windows late-bound COM client parity

References:

- [WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-10_IDISPATCH_LATEBOUND_COM_COMPLETION.md)
- [WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md)
- [COM_CLIENT_SERVER_SCOPE_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\COM_CLIENT_SERVER_SCOPE_V1.md)

Depends on:

- architectural COM boundary already closed under `IP-04`

Unblocks:

- `IP-07` COM event parity lanes
- `IP-08` host-returned COM breadth
- `IP-06` outward `IDispatch` parity expectations

Closeout checklist:

- [ ] close remaining natural/default-member syntax gaps for non-metadata-backed late-bound bindings
- [ ] close the remaining non-`IDispatch` interface-pointer handling policy:
  - support the intended lanes, or
  - reject them deterministically with stable diagnostics
- [ ] close remaining SAFEARRAY legality gaps:
  - non-`IDispatch` element handling
  - multi-dimensional handling
  - unsupported typed-array handling
- [ ] close remaining `ArgErr` / `ExcepInfo` / `VarResult` fidelity gaps for external/native lanes
- [ ] close the remaining practical Office automation lanes that still keep the area described as a subset
- [ ] ensure runtime-string member/default-member/property intent is either executable or diagnostic across the scoped parity target
- [ ] remove bounded-subset language from the `IP-03` row and blockers for any lane that still belongs to `IP-03`
- [ ] record only oracle/formal residuals, if any, under `IP-10` / `IP-11`

Closure test for `IP-03`:

- the `IP-03` row no longer describes the area as a recoverable subset,
- [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) no longer carries a late-bound COM parity blocker owned by `IP-03`,
- the workset documents no longer carry live late-bound parity debt inside `IP-03`.

### `IP-05` Windows early-bound COM and type-library parity

References:

- [WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-11_COM_REFERENCE_FACADE_AND_TYPELIB_BINDING_COMPLETION.md)
- [WORKSET_2026-03-18_IP-05A_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-05A_EXECUTION_CHECKLIST.md)
- [COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md)

Depends on:

- metadata facade and imported-binding floor already established
- should be closed before final `IP-08` host-returned COM breadth and before `IP-06` outward typelib claims

Unblocks:

- `IP-08` host-returned imported-member breadth
- `IP-06` typelib publication and outward imported-member parity claims
- part of `IP-07` imported-event/`WithEvents` ownership boundaries

Closeout checklist:

- [ ] close richer typelib/member coverage beyond the current controlled imported subset
- [ ] close broader imported member/property/default-member lowering across the intended parity matrix
- [ ] close imported event behavior to the intended scoped target:
  - execute supported lanes, or
  - reject unsupported lanes deterministically without leaving them as vague boundary debt
- [ ] close broader Office/Excel object-model behavior still outside the current imported subset
- [ ] eliminate ad hoc “supported imported subset” language from the `IP-05` row
- [ ] ensure imported declarations, members, default members, and event-related surfaces all have either executable proof or deterministic diagnostics in the scoped target
- [ ] update blockers/worklist so no live early-bound parity gap remains under `IP-05`

Closure test for `IP-05`:

- the `IP-05` row no longer describes the area as a constrained or narrower subset,
- imported member/property/default-member/event behavior is explicit across the intended early-bound target,
- no live blocker entry remains owned by `IP-05`.

### `IP-09` Declare/native marshaling parity

References:

- [HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md)
- [HAL_CONTRACT_CLAUSE_CATALOG_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\HAL_CONTRACT_CLAUSE_CATALOG_V1.md)

Depends on:

- can progress in parallel with `IP-03` / `IP-05`
- should be settled before final `IP-06` closure

Unblocks:

- `IP-06` outward server/export parity
- remaining native/host ABI truth needed for broader parity claims

Closeout checklist:

- [ ] close the full Automation legality matrix for the scoped declare/native target
- [ ] close pointer-string lane behavior
- [ ] close byref writeback behavior
- [ ] close richer native ABI shape support or deterministic rejection
- [ ] close `IDispatch::Invoke` output obligations that are owned by marshaling policy rather than client/server feature work
- [ ] remove “deterministic subset” language from the `IP-09` row for any lane still owned by `IP-09`
- [ ] make the remaining unsupported ABI shapes explicitly diagnostic, not silently partial

Closure test for `IP-09`:

- [HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md) no longer describes a deliberately narrow declare/native subset for the scoped target,
- the `IP-09` row no longer says richer ABI shapes/writeback/output obligations remain open,
- no live blocker entry remains owned by `IP-09`.

### `IP-07` Event runtime parity

References:

- [WORKSET_2026-03-08_EVENTS_PARITY_CLOSURE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-08_EVENTS_PARITY_CLOSURE.md)

Depends on:

- stable `IP-03` COM event transport/runtime semantics
- stable imported/event boundaries from `IP-05`
- stable host ingress ownership boundaries now that `IP-08` no longer owns the residual event gap

Unblocks:

- final `IP-08` host closure
- final event parity claims in the program
- part of `IP-06` outward event/source expectations

Closeout checklist:

- [ ] close full `WithEvents` instance graph semantics
- [ ] close unified host ingress parity
- [ ] close complete COM event parity lanes
- [ ] close event subscription/ownership/routing behavior for the scoped target
- [ ] ensure missing-handler, wrong-arity, unsupported-subscription, and routing diagnostics are stable where unsupported
- [ ] remove the remaining event-runtime residuals from [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
- [ ] remove “baseline pass completed but residual parity remains” language from the `IP-07` row

Closure test for `IP-07`:

- the `IP-07` row is no longer `in-progress`,
- [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) no longer says the event-runtime residuals remain under `IP-07`,
- the event workset no longer carries open instance-graph, host-ingress, or COM-event debt.

### `IP-08` Host project / Office-style hosting parity

References:

- [WORKSET_2026-03-19_IP-08A_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-19_IP-08A_EXECUTION_CHECKLIST.md)
- [WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md)
- [HOSTING_PROJECT_TOOLING_PROPOSAL.md](C:\Work\DnaCalc\OxVba\docs\spec\HOSTING_PROJECT_TOOLING_PROPOSAL.md)

Depends on:

- `IP-03` for settled host-returned late-bound COM behavior
- `IP-05` for settled host-returned imported-member breadth
- `IP-07` for the remaining event-runtime residuals already assigned away from `IP-08`

Unblocks:

- final host/Office-style parity claim
- `IP-06` host policy model clarity

Closeout checklist:

- [ ] close the remaining supported host root/global/project behavior matrix across:
  - assignment intent
  - invoke shape
  - precedence rules
- [ ] close the remaining host-returned COM-object matrix across the intended imported member/property/default-member breadth
- [ ] close the remaining active-project vs host-root precedence rows that are still only implied
- [ ] close the remaining plain-project vs host-root precedence rows that are still only implied
- [ ] ensure any unsupported host-root or host-returned COM rows fail deterministically rather than remaining outside the matrix
- [ ] make [WORKSET_2026-03-19_IP-08A_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-19_IP-08A_EXECUTION_CHECKLIST.md) and [WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md) fully checkable to completion
- [ ] remove the remaining “broader imported member/property/default-member breadth” language from the `IP-08` row and blockers

Closure test for `IP-08`:

- both `IP-08A` and `IP-08B` checklists are fully satisfied,
- [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) no longer carries a host project / Office-style hosting parity blocker,
- the `IP-08` row can be described without bounded-breadth caveats inside its scoped target.

### `IP-06` Windows COM server/export parity

References:

- [COM_CLIENT_SERVER_SCOPE_V1.md](C:\Work\DnaCalc\OxVba\docs\spec\COM_CLIENT_SERVER_SCOPE_V1.md)
- [WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-08_VBA71_WINDOWS_OFFICE_FULL_COMPLIANCE.md)

Depends on:

- `IP-03` for outward dispatch semantics
- `IP-05` for typelib/member/reference semantics
- `IP-07` for event/source semantics
- `IP-08` for host policy model clarity
- `IP-09` for ABI/marshaling truth

Unblocks:

- final COM client/server parity claim in the scoped program

Closeout checklist:

- [ ] deliver the intended class exposure model
- [ ] deliver typelib publication to the intended scoped target
- [ ] deliver outward `IDispatch` parity to the intended scoped target
- [ ] deliver the intended host policy model for server/export behavior
- [ ] close any outward event/source exposure required by the scoped parity target
- [ ] prove client/server end-to-end behavior on the intended supported matrix
- [ ] move the `IP-06` row from `planned` to `closed` rather than leaving it as accepted future work

Closure test for `IP-06`:

- the `IP-06` row is no longer `planned`,
- the server/export scope docs no longer say behavior is below parity target for the intended scope,
- no live blocker or deferred “major unfinished domain” note remains owned by `IP-06`.

## Final closure sweep

After the per-area items above are complete, run this final sweep:

- [ ] re-read every `IP-03` / `IP-05` / `IP-06` / `IP-07` / `IP-08` / `IP-09` row in [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- [ ] remove any remaining bounded-subset or partial-parity language that still belongs to one of those work areas
- [ ] re-read [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) and verify every remaining blocker belongs to some other owner
- [ ] verify no open checklist item remains in any area-specific workset above
- [ ] verify any remaining differential/formal obligation is owned only by `IP-10` / `IP-11`
- [ ] only then mark the area rows `closed`

## Practical note

This checklist is intentionally stricter than a sequencing note.

Its job is not only to suggest what to do next.
Its job is to prevent a false closure where one area is declared finished while another still quietly depends on unfinished COM/host semantics inside the same claim surface.
