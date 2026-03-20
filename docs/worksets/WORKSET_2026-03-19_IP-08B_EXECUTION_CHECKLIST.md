# WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST

Purpose: turn the completed `IP-08A` host foundation into an explicit `IP-08B` parity-breadth checklist for the scoped Office-style hosting surface.

## Scope discipline

- `IP-08A` is already the host-foundation floor. This checklist must not relitigate whether the repo has a working host substrate.
- `IP-08B` owns parity breadth on top of that substrate:
  - richer root/global/project behavior,
  - broader imported member/property/default-member breadth on host-returned COM objects,
  - final host integration with the completed property/event/COM model.
- Keep the workset completion doctrine from `OPERATIONS.md` section `3.1` active: if a scoped parity area is not complete, status remains `in-progress`.

## Exit gate

`IP-08B` is complete only when all of the following are true:

- [ ] The supported host root/global/project behavior matrix is explicit across assignment intent, invoke shape, and precedence rules for the scoped hosting target.
- [ ] The supported host-returned COM-object matrix is explicit across the intended imported member/property/default-member breadth for the scoped hosting target.
- [x] Host callback / event behavior no longer carries `IP-08`-owned parity gaps that belong above the completed `IP-08A` substrate.
- [ ] `CURRENT_BLOCKERS.md` and `IN_PROGRESS_FEATURE_WORKLIST.md` describe only the truly remaining host parity breadth, not missing host foundation semantics.

## Lane matrix

Classify each lane as exactly one of:

- `proved-exec`
- `proved-diagnostic`
- `implemented-unproved`
- `missing-semantics`
- `missing-diagnostic`
- `oracle-needed`

Axes:

1. Receiver family
- host-injected root
- active-project root / same-name local neighbor
- plain referenced-project neighbor
- host-returned native object
- host-returned COM-backed object

2. Exposure / identity mode
- `VB_PredeclaredId`
- `VB_GlobalNamespace`
- no implicit exposure
- same-name local project neighbor
- same-name plain referenced-project neighbor

3. Syntax / intent
- explicit `Set`
- explicit `Let`
- implicit assignment
- explicit `Call`
- bare statement-context
- parenthesized zero-arg
- indexed
- named-argument

4. Result / traffic kind
- scalar getter
- object-valued getter
- scalar setter
- object setter
- imported method/default-member invoke
- event/callback behavior

## Immediate frontier

The next bounded executable neighbors after `IP-08A` are:

- [x] widen host-root object-valued getter assignment-intent evidence beyond typed `Object` targets into the `Variant` matrix
- [x] widen host-root object-valued getter syntax breadth into parenthesized zero-arg named-property getter `Variant` lanes
- [x] widen host-root authoritative default-member object-get assignment-intent evidence into the `Variant` matrix
- [x] widen host-root object-valued getter syntax breadth through the parenthesized authoritative default-member `Variant` neighbor
- [ ] widen host-returned COM imported breadth beyond the currently proved bounded member/property/default-member subset where parity requires it
- [x] capture the remaining host callback / event breadth that still belongs to `IP-08` rather than `IP-07`

Current event/callback boundary:

- the host-backed callback floor now has direct compiler and host evidence for zero/one-argument ingress on referenced `HostInjected` event sources across both `VB_PredeclaredId` and `VB_GlobalNamespace`,
- the same floor now also proves source-instance routing, same-name plain-project precedence on one-argument routes, and deterministic rejection of higher-arity forwarded host ingress on live host-backed source handles,
- remaining event-runtime residuals stay under `IP-07` (`DIV-0004`, `ODG-038`, `ODG-039`, and the remaining COM event parity lanes), not under `IP-08`.

Current host-returned COM breadth boundary:

- same-name plain-project precedence is now explicit for the currently proved imported scalar read-assignment, named-argument read-assignment, positional read-assignment, and compile-time diagnostic lanes on host-returned COM-backed objects,
- active-project same-name `Application` precedence is now also explicit for the matching imported scalar, named-argument, and positional read-assignment lanes, the imported property-put/get, property-putref, indexed-setter, and exception-invoke lanes, the current positional/default-member explicit-`Call` and bare statement-context subsets across both parenthesized and no-paren forms, and the current named-argument explicit-`Call` and bare statement-context subsets across both parenthesized and no-paren forms, on host-returned COM-backed objects,
- the remaining `IP-08B` COM breadth is therefore narrower than the original frontier: richer imported member/property/default-member rows may still remain, but these newer read/diagnostic neighbors are no longer only implied by earlier precedence evidence.
