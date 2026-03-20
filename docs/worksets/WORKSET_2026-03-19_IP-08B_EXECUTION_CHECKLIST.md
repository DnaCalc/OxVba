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
- [ ] Host callback / event behavior no longer carries `IP-08`-owned parity gaps that belong above the completed `IP-08A` substrate.
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
- [ ] widen host-root object-valued getter syntax breadth beyond the current named non-indexed plus parenthesized zero-arg floor where parity requires it
- [ ] widen host-returned COM imported breadth beyond the currently proved bounded member/property/default-member subset where parity requires it
- [ ] capture the remaining host callback / event breadth that still belongs to `IP-08` rather than `IP-07`
