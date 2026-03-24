# WORKSET_2026-03-24_INITIAL_SCOPE_EXECUTION_CHECKLIST

## Purpose

Turn the remaining items from `docs/INITIAL_SCOPE_STATUS_2026-03-24.md` into an explicit execution order for initial-scope closure.

This checklist keeps the current gate honest:

- the terminal gate is driven by the must-do evidence and formal items,
- the repo-fixable engine gaps are real closure work but are not currently described as terminal-gate blockers,
- doctrine from `OPERATIONS.md` section `3.1` remains binding.

## Governing sources

- [CHARTER.md](C:\Work\DnaCalc\OxVba\CHARTER.md)
- [OPERATIONS.md](C:\Work\DnaCalc\OxVba\OPERATIONS.md)
- [MACH1000_PLAN.md](C:\Work\DnaCalc\OxVba\MACH1000_PLAN.md)
- [INITIAL_SCOPE_STATUS_2026-03-24.md](C:\Work\DnaCalc\OxVba\docs\INITIAL_SCOPE_STATUS_2026-03-24.md)
- [WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md)
- [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
- [DEFERRED_ORACLE_GATES.csv](C:\Work\DnaCalc\OxVba\docs\evidence\conformance\DEFERRED_ORACLE_GATES.csv)
- [DEFERRED_GATES.md](C:\Work\DnaCalc\OxVba\docs\evidence\formal\DEFERRED_GATES.md)

## Exit gate

Initial-scope closure is reached only when all of the following are true:

- [ ] The must-do evidence items in `INITIAL_SCOPE_STATUS_2026-03-24.md` are either closed with linked evidence or explicitly deferred with exact unblock steps and rationale.
- [ ] `DG-V2-001` is no longer left in an indeterminate running state for this closure pass; it is either folded or explicitly deferred.
- [ ] `CURRENT_BLOCKERS.md` and the initial-scope status doc describe only the true remaining external closure constraints.
- [ ] The repo-fixable engine gaps are either fixed with tests or explicitly restated as non-blocking residuals with exact next steps.

## Work partition

### Track A. Terminal-gate evidence closure

Owned items:

- [ ] `ODG-044`
- [ ] `ODG-045`
- [ ] `ODG-046`
- [ ] `ODG-030`
- [ ] `ODG-031`
- [ ] `DG-V2-001`

Execution order:

1. Verify the oracle-template and harness readiness for `ODG-044..046`.
2. Record the exact Excel oracle-session packet:
   - capture files,
   - host prerequisites,
   - evidence destinations,
   - foldback targets.
3. Decide `ODG-030/031` honestly:
   - close now if the required COM and typelib harness exists and is runnable,
   - otherwise defer explicitly with exact missing infrastructure and unblocking steps.
4. Reconcile `DG-V2-001`:
   - fold if a completed remote result exists,
   - otherwise move it to explicit defer state with rationale and unblock steps.

Acceptance:

- [ ] No terminal-gate item remains as an implicit “later”.
- [ ] Every open item has either evidence, an explicit defer state, or an actionable scheduled next step owned by this run.

### Track B. Repo-fixable engine closure

Owned items:

- [x] Single-line `If` with statement
- [x] ParamArray named arguments
- [ ] Dynamic dispatch named/omitted args

Execution order:

1. Single-line `If` with statement
2. ParamArray named arguments
3. Dynamic dispatch named/omitted args

Why this order:

- single-line `If` is a local resolver/project-compiler gap with no external dependency,
- ParamArray named-arg support is a contained call-mapping expansion on top of already-proved named and optional argument behavior,
- dynamic dispatch named/omitted args already reach the VM request model and should close last because they touch shared project-object runtime routing.

Acceptance:

- [ ] Each engine item has at least one focused compiler or host regression test.
- [ ] Any residual unsupported subset is explicit and narrow rather than a silent fallback.

## Current execution notes

### B1. Single-line `If` with statement

Current gap:

- the project resolver only recognizes multiline `If ... Then` headers in the main block walker,
- single-line `If cond Then stmt` falls through the line-based parser and becomes unsupported in the project path.

Planned closure:

- [x] add single-line `If` parsing in the project resolver,
- [x] cover the assignment form (`If x = 1 Then x = x + 2`),
- [x] cover the statement form called out in current status (`If cond Then Err.Raise N`),
- [x] verify project execution, not just source compilation.

### B2. ParamArray named arguments

Current gap:

- compiler typecheck still hard-rejects named args for `ParamArray` procedures.

Planned closure:

- [x] replace the current subset rejection with parameter mapping that permits named fixed parameters on `ParamArray` procedures while keeping named `ParamArray` targets rejected,
- [x] update the old current-subset rejection tests,
- [x] verify omission and order diagnostics still behave correctly.

### B3. Dynamic dispatch named/omitted args

Current gap:

- VM request construction already preserves `name` and omission metadata,
- project-object dynamic dispatch still rejects those shapes during native routing.

Planned closure:

- [ ] teach project dynamic routing to canonicalize named and omitted arguments against the selected member metadata,
- [ ] preserve current ambiguity and arity diagnostics,
- [ ] verify shared behavior on native project objects and host-returned object paths that use the same route.

## Verification lanes

For each code slice:

- [ ] `cargo test -p oxvba-compiler --quiet`
- [ ] `cargo test -p oxvba-host <focused test name> --quiet`

For each doc and evidence foldback slice:

- [ ] `./scripts/check-governance.ps1`

For the combined closeout pass:

- [ ] `./scripts/meta-check.ps1 -Fast -NoArtifacts`

## Status discipline

- Do not mark this workset `closed` while any terminal-gate item still depends on unscheduled oracle capture or unresolved formal foldback.
- Do not describe any of the three engine items as complete until code, tests, and the status surface all agree.
