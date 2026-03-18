# WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST

## Purpose

Turn `IP-02` from a rolling frontier into an explicit execution checklist that can be used to prove honest completion under the workset completion doctrine.

`IP-02` remains `in-progress` until every item in this document is either:
- proved executable with compiler + host evidence, or
- proved intentionally unsupported with deterministic diagnostics and recorded rationale.

## Governing sources

Primary contract sources:
- [OPERATIONS.md](C:\Work\DnaCalc\OxVba\OPERATIONS.md)
- [MACH1000_PLAN.md](C:\Work\DnaCalc\OxVba\MACH1000_PLAN.md)
- [PROFILE_LADDER_2026-03-08_MACH1000_V467_V620_VBA71_WINDOWS_OFFICE_COMPLIANCE.md](C:\Work\DnaCalc\OxVba\docs\worksets\PROFILE_LADDER_2026-03-08_MACH1000_V467_V620_VBA71_WINDOWS_OFFICE_COMPLIANCE.md)
- [WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md)
- [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
- [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)

Binding doctrine pulled from those sources:
- `IP-02` must satisfy the `DG-03` semantic model, not just isolated passing examples.
- `IP-02A` acceptance requires property/default-member behavior to stop being described as partial/shared debt in [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md).
- Partial subsets must remain `in-progress`; do not use completion language for bounded subsets.

## Exit gate

`IP-02` is complete only when all of the following are true:

- [ ] One authoritative property/default-member semantic model is in force across:
  - binder,
  - compiler lowering,
  - VM dynamic dispatch,
  - COM late-bound bridge where applicable,
  - early-bound metadata-backed calls,
  - future COM server/export behavior.
- [ ] `Set` vs `Let` intent is explicit and enforced for all supported native/property/default-member lanes in scope.
- [ ] Default-member identity and fallback policy are explicit for:
  - authoritative native default members,
  - non-authoritative native fallback,
  - metadata-backed COM/default-member consumers where `IP-02` depends on them.
- [ ] Indexed/default-property call-vs-value behavior is complete in the supported native scope.
- [ ] Every supported `IP-02` lane has compiler proof and host VM/JIT proof.
- [ ] Every unsupported `IP-02` lane fails deterministically with a stable diagnostic.
- [ ] [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) no longer lists a live `IP-02` semantic gap.
- [ ] The `IP-02` row in [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) no longer relies on bounded-subset language for open `Set`/`Let`, default-member, or call-vs-value behavior.

## Lane matrix

Each lane must be classified as exactly one of:
- `proved-exec`
- `proved-diagnostic`
- `implemented-unproved`
- `missing-semantics`
- `missing-diagnostic`
- `oracle-needed`

Each lane is identified by these axes:

1. Receiver form
- named property
- default member

2. Authority mode
- authoritative identity
- non-authoritative single visible candidate
- non-authoritative ambiguous candidates
- non-authoritative no viable candidate

3. Shape
- scalar / non-indexed
- indexed
- zero-arg parenthesized
- no-parentheses arguments

4. Context
- read-assignment RHS
- explicit `Let` read-assignment
- explicit `Set` read-assignment
- implicit assignment
- explicit `Let` write-assignment
- explicit `Set` property assignment
- statement-context getter
- explicit `Call` getter

5. Value kind
- scalar getter/value source
- object getter/value source

6. Target kind
- scalar target
- `Object` target
- `Variant` target

## Current proved floor

Already evidenced in the repo today:

- authoritative native member/default-member `Property Get` / `Property Let` / `Property Set`
- authoritative indexed member/default-member `Get` / `Let` / `Set`
- authoritative statement-context, `Call`, zero-arg parenthesized, and no-parentheses-argument getter routes for the proven native subset
- bounded explicit `Set` / `Let` preservation through native PMR/default-member read-assignment rewrites
- bounded authoritative object-returning native property/default-member getter read-assignment into `Variant` targets for explicit `Set`, explicit `Let`, and implicit assignment across named, zero-arg parenthesized, indexed, and authoritative default-member syntax
- bounded `CreateObject(...)` assignment-intent proof surface for explicit `Set`, explicit `Let`, and implicit assignment on current typed `Object` / `Variant` / scalar target lanes
- non-authoritative single-visible-candidate native default-member fallback for scalar/indexed `Get` / `Let` / `Set`, statement-context getters, explicit `Call` getters, no-parentheses-argument getters, bounded explicit `Set` / `Let` read-assignment, bare/parenthesized/indexed object-returning `Variant`-target read-assignment neighbors, and parenthesized zero-arg getter/read-assignment neighbors where already landed
- non-authoritative ambiguous native default-member getter / let-assignment / property-set diagnostics via `PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS` across scalar/indexed read-assignment plus statement-context, explicit `Call`, no-parentheses-argument, and zero-arg parenthesized getter contexts where applicable
- non-authoritative missing native default-member getter / let-assignment / property-set diagnostics via `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING` across scalar/indexed read-assignment plus statement-context, explicit `Call`, no-parentheses-argument, zero-arg parenthesized getter, and indexed `Property Set` contexts where applicable

## Remaining checklist by closure domain

### A. Non-authoritative default-member resolution

- [x] Add deterministic `no viable candidate` diagnostics for native non-authoritative default-member use instead of silent rewrite escape.
- [x] Prove scalar getter `no viable candidate` behavior.
- [x] Prove indexed getter `no viable candidate` behavior.
- [x] Prove scalar and indexed `Let` assignment `no viable candidate` behavior.
- [x] Prove `Property Set` `no viable candidate` behavior.
- [x] Prove indexed `Property Set` `no viable candidate` behavior.
- [x] Prove statement-context getter `no viable candidate` behavior.
- [x] Prove explicit `Call` getter `no viable candidate` behavior.
- [x] Prove zero-arg parenthesized statement/`Call` getter `no viable candidate` behavior.
- [x] Decide and prove indexed ambiguous lanes where not already locked.
- [x] Decide and prove call/statement/parenthesized ambiguity neighbors where not already locked.

### B. Call-vs-value parity

- [ ] Enumerate every native PMR/default-member getter syntax form and mark it `proved-exec` or `proved-diagnostic`.
- [ ] Verify no silent fallthrough remains for unsupported syntax forms.
- [ ] Sweep parenthesized/indexed/default-member combinations still only implied by adjacent lanes.
- [x] Sweep no-parentheses-argument forms against authoritative and non-authoritative receivers separately.
- [ ] Record any Office-observed divergence before widening semantics.

### C. `Set` vs `Let` intent parity

- [ ] Build the explicit source-target table for:
  - scalar source,
  - object source,
  - object-producing call result,
  - property/default-member getter result.
- [ ] For each source-target pair, mark:
  - implicit assignment,
  - explicit `Let`,
  - explicit `Set`,
  - expected compile-time rejection where applicable.
- [ ] Prove remaining scalar/object mismatch diagnostics not yet directly covered.
- [ ] Prove remaining object-getter read-assignment lanes across `Object` / `Variant` / scalar targets or reject them deterministically.
- [ ] Remove any acceptance that still survives by accidental rewrite rather than explicit policy.

### D. Evidence and closure hygiene

- [ ] For every new lane, add compiler proof in:
  - [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs),
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs),
  - [typecheck.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\typecheck.rs),
  as appropriate.
- [ ] For every new lane, add host VM/JIT proof in [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs).
- [ ] Update [IMPLEMENTATION_LOG.md](C:\Work\DnaCalc\OxVba\docs\IMPLEMENTATION_LOG.md) after each landed slice.
- [ ] Keep [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) and [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) honest after each slice.

## Run order

The recommended execution order is:

1. non-authoritative `no viable candidate` diagnostics
2. remaining non-authoritative ambiguity neighbors
3. remaining call-vs-value syntax sweep
4. remaining `Set` / `Let` source-target sweep
5. final `IP-02` proof sweep and doc cleanup

## Per-slice execution rule

For each slice:

1. implement the semantic or diagnostic change
2. add compiler proof
3. add host proof
4. update [IMPLEMENTATION_LOG.md](C:\Work\DnaCalc\OxVba\docs\IMPLEMENTATION_LOG.md)
5. run:
   - `cargo fmt --all`
   - `cargo test -p oxvba-com -p oxvba-compiler -p oxvba-hal -p oxvba-host --quiet`
   - `./scripts/check-governance.ps1`
   - `./scripts/meta-check.ps1 -Fast -NoArtifacts`
6. commit
7. push
8. continue to the next unchecked item

## Active next slice

First checklist-driven target:

- Landed in the first slice:
  - native non-authoritative default-member `no viable candidate` now fails deterministically with `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING`
  - compiler + host evidence exists for scalar/indexed getter, scalar/indexed `Let`, and scalar/indexed `Property Set`
  - compiler + host evidence also exists for statement-context, explicit `Call`, and no-parentheses-argument getter forms in scalar/indexed shape plus the zero-arg parenthesized getter neighbors
  - compiler + host evidence now also exists for ambiguous scalar/indexed getter, `Let`, and `Property Set` diagnostics across read-assignment plus statement-context, explicit `Call`, no-parentheses-argument, and zero-arg parenthesized getter contexts where applicable
- Next unresolved neighbors:
  - broader call-vs-value syntax enumeration, especially any remaining silent-fallthrough or receiver-mode distinctions outside the now-proved no-parentheses subset
  - broader `Set` vs `Let` source-target sweep beyond the now-proved named object-property-get `Variant` lanes
