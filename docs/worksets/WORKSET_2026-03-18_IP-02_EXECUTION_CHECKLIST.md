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

- [x] One authoritative property/default-member semantic model is in force across:
  - binder,
  - compiler lowering,
  - VM dynamic dispatch,
  - COM late-bound bridge where applicable,
  - early-bound metadata-backed calls,
  - future COM server/export behavior.
- [x] `Set` vs `Let` intent is explicit and enforced for all supported native/property/default-member lanes in scope.
- [x] Default-member identity and fallback policy are explicit for:
  - authoritative native default members,
  - non-authoritative native fallback,
  - metadata-backed COM/default-member consumers where `IP-02` depends on them.
- [x] Indexed/default-property call-vs-value behavior is complete in the supported native scope.
- [x] Every supported `IP-02` lane has compiler proof and host VM/JIT proof.
- [x] Every unsupported `IP-02` lane fails deterministically with a stable diagnostic.
- [x] [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) no longer lists a live `IP-02` semantic gap.
- [x] The `IP-02` row in [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) no longer relies on bounded-subset language for open `Set`/`Let`, default-member, or call-vs-value behavior.

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
- bounded authoritative `Object`-target rejection for object-returning native property/default-member getter read-assignment on explicit `Let` and implicit assignment across named, zero-arg parenthesized, indexed, and authoritative default-member syntax
- bounded single-visible-candidate non-authoritative Object-target rejection for object-returning native default-member getter read-assignment on explicit Let and implicit assignment across bare, zero-arg parenthesized, and indexed syntax
- bounded ambiguous/missing non-authoritative object-valued default-member read-assignment diagnostics on the source-resolution path for explicit `Set`, explicit `Let`, and implicit assignment to `Object` targets across bare, zero-arg parenthesized, and indexed syntax
- bounded ambiguous/missing non-authoritative object-valued default-member read-assignment diagnostics on the source-resolution path for explicit `Set`, explicit `Let`, and implicit assignment to typed `Variant` targets across bare, zero-arg parenthesized, and indexed syntax
- bounded ambiguous/missing non-authoritative object-valued default-member read-assignment diagnostics on the source-resolution path for explicit `Set`, explicit `Let`, and implicit assignment to scalar targets across bare, zero-arg parenthesized, and indexed syntax
- bounded no-parentheses getter RHS read-assignment rejection via the compile-time `unsupported statement` surface across named, authoritative default-member, and single-visible-candidate non-authoritative default-member receivers under explicit `Set`, explicit `Let`, and implicit assignment for typed `Variant`, `Object`, and scalar targets
- bounded plain scalar-source assignment-intent proof surface for explicit `Set`, explicit `Let`, and implicit assignment on current typed scalar / `Variant` / `Object` target lanes
- bounded plain object-source assignment-intent proof surface for explicit `Set`, explicit `Let`, and implicit assignment on current typed `Object` / `Variant` / scalar target lanes
- bounded plain declared-`Variant` source assignment-intent proof surface with runtime payload validation across the current typed `Variant` / `Object` / scalar target lanes for both scalar-payload and object-payload shapes
- bounded scalar-target rejection for object-returning native property/default-member getter read-assignment across named, zero-arg parenthesized, indexed, authoritative default-member, and landed single-candidate non-authoritative default-member syntax for explicit `Set`, explicit `Let`, and implicit assignment
- bounded scalar-typed native property/default-member getter read-assignment rejection for explicit `Set` across named, zero-arg parenthesized, indexed, authoritative default-member, and landed single-candidate non-authoritative default-member syntax for typed `Variant`, `Object`, and scalar targets
- bounded scalar-typed native property/default-member getter read-assignment execution for explicit `Let` and implicit assignment into typed `Variant` and scalar targets across named, zero-arg parenthesized, indexed, authoritative default-member, and landed single-candidate non-authoritative default-member syntax
- bounded scalar-typed native property/default-member getter read-assignment rejection for explicit `Let` and implicit assignment on typed `Object` targets across named, zero-arg parenthesized, indexed, authoritative default-member, and landed single-candidate non-authoritative default-member syntax
- bounded `CreateObject(...)` assignment-intent proof surface for explicit `Set`, explicit `Let`, and implicit assignment on current typed `Object` / `Variant` / scalar target lanes
- non-authoritative single-visible-candidate native default-member fallback for scalar/indexed `Get` / `Let` / `Set`, statement-context getters, explicit `Call` getters, no-parentheses-argument getters, bounded explicit `Set` / `Let` read-assignment, bare/parenthesized/indexed object-returning `Variant`-target read-assignment neighbors, and parenthesized zero-arg getter/read-assignment neighbors where already landed
- non-authoritative ambiguous native default-member getter / let-assignment / property-set diagnostics via `PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS` across scalar/indexed read-assignment plus statement-context, explicit `Call`, no-parentheses-argument, and zero-arg parenthesized getter contexts where applicable
- non-authoritative missing native default-member getter / let-assignment / property-set diagnostics via `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING` across scalar/indexed read-assignment plus statement-context, explicit `Call`, no-parentheses-argument, zero-arg parenthesized getter, and indexed `Property Set` contexts where applicable

## Closure audit matrices

### Getter syntax classification in the current native scope

- named property getters
  - statement-context `widget.Value`, `widget.Value()`, `widget.Value(x)`: `proved-exec`
  - explicit `Call` `Call widget.Value`, `Call widget.Value()`, `Call widget.Value(x)`: `proved-exec`
  - no-parentheses-argument `widget.Value x`: `proved-exec`
  - RHS read-assignment no-parentheses forms under explicit `Set`, explicit `Let`, and implicit assignment: `proved-diagnostic` on the stable `unsupported statement` surface
- authoritative default-member getters
  - statement-context `widget`, `widget()`, `widget(x)`: `proved-exec`
  - explicit `Call` `Call widget`, `Call widget()`, `Call widget(x)`: `proved-exec`
  - no-parentheses-argument `widget x`: `proved-exec`
  - RHS read-assignment no-parentheses forms under explicit `Set`, explicit `Let`, and implicit assignment: `proved-diagnostic` on the stable `unsupported statement` surface
- non-authoritative single-visible-candidate default-member getters
  - statement-context `widget`, `widget()`, `widget(x)`: `proved-exec`
  - explicit `Call` `Call widget`, `Call widget()`, `Call widget(x)`: `proved-exec`
  - no-parentheses-argument `widget x`: `proved-exec`
  - RHS read-assignment no-parentheses forms under explicit `Set`, explicit `Let`, and implicit assignment: `proved-diagnostic` on the stable `unsupported statement` surface
- non-authoritative ambiguous or missing default-member receivers
  - scalar/indexed read-assignment, statement-context, explicit `Call`, no-parentheses-argument, zero-arg parenthesized getter, and indexed `Property Set` neighbors: `proved-diagnostic`
  - stable diagnostics are:
    - `PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS`
    - `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING`

### Assignment-intent matrix in the current native scope

- plain scalar source
  - explicit `Set`: `proved-diagnostic` for scalar, `Object`, and `Variant` targets
  - explicit `Let`: `proved-exec` for scalar and `Variant`; `proved-diagnostic` for `Object`
  - implicit assignment: `proved-exec` for scalar and `Variant`; `proved-diagnostic` for `Object`
- plain `Object` source variable
  - explicit `Set`: `proved-exec` for `Object` and `Variant`; `proved-diagnostic` for scalar
  - explicit `Let`: `proved-exec` for `Variant`; `proved-diagnostic` for `Object` and scalar
  - implicit assignment: `proved-exec` for `Variant`; `proved-diagnostic` for `Object` and scalar
- object-producing call result
  - explicit `Set`: `proved-exec` for `Object` and `Variant`; `proved-diagnostic` for scalar
  - explicit `Let`: `proved-exec` for `Variant`; `proved-diagnostic` for `Object` and scalar
  - implicit assignment: `proved-exec` for `Variant`; `proved-diagnostic` for `Object` and scalar
- plain declared-`Variant` source variable
  - runtime payload validation now makes the scalar-payload and object-payload rows explicit instead of accidental:
    - object payload follows the current object-source matrix
    - scalar payload follows the current scalar-source matrix
- scalar getter result
  - explicit `Set`: `proved-diagnostic` for scalar, `Object`, and `Variant` targets
  - explicit `Let`: `proved-exec` for scalar and `Variant`; `proved-diagnostic` for `Object`
  - implicit assignment: `proved-exec` for scalar and `Variant`; `proved-diagnostic` for `Object`
- object getter result
  - explicit `Set`: `proved-exec` for `Object` and `Variant`; `proved-diagnostic` for scalar
  - explicit `Let`: `proved-exec` for `Variant`; `proved-diagnostic` for `Object` and scalar
  - implicit assignment: `proved-exec` for `Variant`; `proved-diagnostic` for `Object` and scalar
  - coverage spans named, zero-arg parenthesized, indexed, authoritative default-member, and non-authoritative single-visible-candidate default-member syntax in the supported native scope

### Closure audit outcome

- No remaining native PMR/default-member getter syntax form in the supported scope is left unclassified after the compiler/host sweep.
- No remaining source-target pair in the current `Set`/`Let` matrix depends on accidental rewrite behavior.
- Remaining non-metadata-backed late-bound default-member recovery belongs to `IP-03`, not `IP-02`.
- Remaining oracle and formal program gates belong to `IP-10` / `IP-11`, not to the scoped `IP-02` native-property closure target.

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

- [x] Enumerate every native PMR/default-member getter syntax form and mark it `proved-exec` or `proved-diagnostic`.
- [x] Verify no silent fallthrough remains for unsupported syntax forms.
- [x] Sweep parenthesized/indexed/default-member combinations still only implied by adjacent lanes.
- [x] Sweep no-parentheses-argument forms against authoritative and non-authoritative receivers separately.
- [x] Record any Office-observed divergence before widening semantics.

### C. `Set` vs `Let` intent parity

- [x] Build the explicit source-target table for:
  - scalar source,
  - object source,
  - object-producing call result,
  - property/default-member getter result.
- [x] For each source-target pair, mark:
  - implicit assignment,
  - explicit `Let`,
  - explicit `Set`,
  - expected compile-time rejection where applicable.
- [x] Prove remaining scalar/object mismatch diagnostics not yet directly covered.
- [x] Prove remaining object-getter read-assignment lanes across `Object` / `Variant` / scalar targets or reject them deterministically.
- [x] Remove any acceptance that still survives by accidental rewrite rather than explicit policy.

### D. Evidence and closure hygiene

- [x] For every new lane, add compiler proof in:
  - [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs),
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs),
  - [typecheck.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\typecheck.rs),
  as appropriate.
- [x] For every new lane, add host VM/JIT proof in [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs).
- [x] Update [IMPLEMENTATION_LOG.md](C:\Work\DnaCalc\OxVba\docs\IMPLEMENTATION_LOG.md) after each landed slice.
- [x] Keep [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) and [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) honest after each slice.

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

## Closure note

Checklist audit result:

- the native/property/default-member `DG-03` scope is now fully classified as either executable or intentionally unsupported with deterministic diagnostics,
- no live `IP-02` semantic blocker remains,
- `IP-02` can therefore be closed without folding late-bound COM default-member parity (`IP-03`) or program-level oracle/formal gates (`IP-10` / `IP-11`) into this scope.

