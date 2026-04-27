# Workset: V0.2 Scope Roundout Execution

Date: 2026-04-06
Owner: Codex
Status: in-progress

## Purpose

Define and execute the next product-shaping scope for OxVba after the current
`v0.1.x` delivery line: remove the remaining legacy carrier seams, round out the
current runtime/language-service/platform boundary truth, and force explicit
architecture decisions where the product is still living on bounded or
transitional behavior.

This workset is not a generic roadmap note. It is the execution owner for the
current accepted `v0.2` roundout scope.

Per `OPERATIONS.md`, this workset does not use closure language for partial
subsets. Every lane below remains `in-progress` until its declared parity or
decision target is actually satisfied.

## V0.2 Target Scope

`v0.2` is the roundout target for the current OxVba engine, not a claim of full
VBA/Office parity. The target is:

- remove remaining legacy internal carrier seams that distort product truth
- stabilize the runtime architecture around explicit canonical value contracts
- raise late-bound COM behavior from bounded support to an honestly specified
  and executable parity target
- complete the remaining obvious language/runtime coverage holes that still make
  the supported scope feel artificially narrow
- round out the language-service and editor-consumption story enough that OxIde,
  `oxvba-lsp`, and the VS Code extension are first-class, tested consumers
- establish a real native-compilation plan and validation scaffold rather than
  leaving the native-image path as a future placeholder
- put performance and security measurement/hardening on continuous footing
- broaden cross-host validation outside the Windows-primary lane without
  pretending that non-Windows COM is part of this release target

## Current Motivating Truth

The current repo truth already states several important bounded or unfinished
surfaces that this workset treats as active delivery scope rather than passive
documentation:

- legacy compat-slot projection still survives as a snapshot/debug/interop seam
- late-bound `IDispatch` parity is still below VBA/Excel behavior
- some date-string coercion/parsing remains intentionally narrow
- internal runtime representation has now been substantially decided by the
  completed value-model migration workset:
  - canonical string storage is BSTR-aligned
  - canonical Variant/SAFEARRAY carriers are retained as the primary value
    substrate for scoped runtime lanes
  - remaining `RuntimeValue` surfaces are compatibility contracts, not internal
    storage truth
  - broad native struct-overlay and unconstrained UDT-byref ABI parity remain
    explicitly outside the current migration scope
- language services exist, but OxIde, `oxvba-lsp`, and the VS Code surface are
  still bounded and not yet first-class platform consumers
- native compilation is still future-facing rather than an active shipped lane
- performance comparison against VBA is not yet under continuous scaffold
- non-primary hosts exist architecturally, but active validation breadth remains
  thinner than the Windows-primary lane

## Explicit In-Scope Families

### 1. Compat-Slot Excision

The remaining legacy compat-slot projection lane must be removed completely,
not merely narrowed further.

This includes:

- snapshot/debug surfaces that still project rich runtime values down to
  integer compat slots
- host/CLI/interop seams that still rely on legacy slot projection as an
  observable contract
- any remaining tests, assertions, docs, and helper APIs that preserve the old
  projection model as a normal execution truth

Completion target:

- OxVba runtime, VM, JIT, host, CLI, debugger, and conformance evidence use the
  rich runtime-value model directly
- any surviving compatibility bridge is explicit, isolated, and externalized as
  a compatibility adapter rather than a core execution seam

### 2. Late-Bound `IDispatch` Parity Clarification and Closure

The current late-bound COM lane must be clarified urgently and then advanced as
a delivery lane.

This includes:

- naming the exact late-bound member-resolution, invocation, default-member,
  property-get/let/set, missing-argument, named-argument, and event-callback
  behaviors that are currently below VBA/Excel truth
- building an executable parity matrix for those behaviors
- deciding which bounded cases are `v0.2` delivery scope and which remain
  explicit unsupported territory
- delivering the in-scope late-bound parity behavior rather than leaving the
  issue as architecture prose

Completion target:

- the late-bound `IDispatch` surface has an explicit supported matrix
- the `v0.2` supported rows are green with VM/JIT/host evidence
- residual unsupported rows are explicit and justified, not fuzzy

### 3. Comprehensive Date-String Parsing and Coercion

The current narrow date-string parsing lane should be rounded out to completion
for the accepted `v0.2` scope.

This includes:

- broad VBA-style string date coercion and parsing shapes
- reconciliation between:
  - `DateValue`
  - `CDate`
  - `IsDate`
  - host-backed date/time functions
- explicit locale/format policy decisions where VBA behavior is ambiguous or
  host-sensitive

Completion target:

- date-string coercion no longer has obvious “not yet supported” holes across
  accepted `v0.2` input shapes
- the exact accepted grammar/locale policy is documented with executable
  evidence

### 4. Internal Representation and Layout Investigation

OxVba needs an explicit architectural decision on whether the internal
representation of strings, variants, dates, interface references, selected
structures, and event payload formats should converge toward bitwise and
contract-level compatibility with VBA 7.1 / OLE Automation.

This investigation must cover:

- strings and whether internal wide-string / `BSTR` alignment is preferable
- `Variant` / `VARIANT` shape and ownership/lifetime implications
- `Date` representation and whether internal carriers should remain merely
  semantically aligned or become wire-compatible
- interface/object identity, pointer/reference semantics, and lifetime model
- structure/event payload layout and whether marshaling can be reduced or
  eliminated by contract-compatible internal forms
- tradeoffs across:
  - interoperability cost
  - engine simplicity
  - safety and ownership complexity
  - cross-platform portability
  - performance

Completion target:

- an explicit architecture decision:
  - converge toward bit/ABI-compatible internal carriers where valuable, or
  - keep OxVba-native internal carriers and formalize the boundary-translation
    doctrine
- a migration plan for the chosen answer
- no ambiguity in docs about whether current marshaling is temporary,
  permanent, or under review

Current rollout note, 2026-04-27:

- the value-model migration workset
  [WORKSET_2026-04-20_VALUE_MODEL_MIGRATION_COMPARISON_AND_PERF_PLAN.md](/C:/Work/DnaCalc/OxVba/docs/worksets/WORKSET_2026-04-20_VALUE_MODEL_MIGRATION_COMPARISON_AND_PERF_PLAN.md)
  is complete
- `v02.5` should consume that completed decision/evidence and decide only the
  remaining V0.2 doctrine gaps rather than reopening the full migration
- `v02.2` remains the next blocking delivery lane because remaining
  compat-slot projection seams still distort product truth

### 5. Language Services Roundout

The language-service family should be rounded out for:

- the direct OxIde consumer surface
- `oxvba-lsp`
- the VS Code extension

This includes:

- direct semantic service APIs that are stable enough for editor consumers
- host/workspace/project loading discipline
- richer semantic queries, navigation, diagnostics, symbol/intellisense-facing
  operations, and edit/update flows
- transport and integration work needed for a usable VS Code surface

Completion target:

- OxIde is a first-class direct consumer of the language-service surface
- `oxvba-lsp` and the VS Code extension expose an honestly documented supported
  `v0.2` subset with active test coverage

### 6. Native Compilation Path

OxVba needs an explicit `v0.2` native-compilation plan.

This workset must:

- decide whether the primary path is Cranelift-based, another backend, or a
  staged hybrid
- define the product target:
  - direct native image,
  - wrapper-hosted compiled artifact,
  - or both in staged order
- identify correctness, ABI, packaging, and deployment obligations
- create the first executable validation scaffold for the chosen path

Completion target:

- the native compilation path is no longer hand-waved as “future”
- there is a concrete chosen direction with measurable next execution gates

### 7. Performance Scaffolding Against VBA

OxVba needs continuous performance scaffolding that compares relevant workloads
against VBA behavior and against OxVba’s own backend combinations.

This includes:

- benchmark corpus design
- Windows Office/VBA comparison harness strategy
- VM vs JIT vs other future backends
- representative host-sensitive workloads
- publishing benchmark methodology and regression thresholds

Completion target:

- benchmark harnesses exist and run reproducibly
- at least an initial comparison corpus against VBA is in place
- performance claims stop being anecdotal

### 8. COM Interop Corpus with Excel and Access/JET

OxVba should start active corpus-building and execution against real COM interop
scenarios centered on Excel and Access/JET.

This includes:

- late-bound and early-bound Excel scenarios
- Access/JET automation and data/object interaction scenarios
- typelib import plus live activation/invoke behaviors
- event, structure, array, and value-marshaling corpus growth

Completion target:

- an explicit interop corpus exists
- results are classified as:
  - works,
  - fails,
  - unsupported
- real Office/Access/JET evidence becomes part of routine conformance truth

### 9. VM and JIT Hardening / Security Pass

The runtimes need a hardening pass focused on invalid inputs, malformed data,
boundary validation, unsafe assumptions, and failure behavior.

This includes:

- bytecode/input validation boundaries
- malformed project/source/runtime inputs
- hostile or nonsensical host/interp payloads
- unsafe code review focus areas
- JIT codegen and runtime helper safety assumptions
- deterministic diagnostics instead of panics, UB, or silent corruption

Completion target:

- the runtimes have an explicit hardening matrix
- hostile/bad-input handling is tested
- safety-sensitive assumptions are documented and exercised

### 10. Non-Primary Host Environment Validation

Non-primary host environments need active testing, especially:

- wasm/web
- Linux
- macOS

This is a host/runtime validation lane, not a COM-parity lane.

Completion target:

- active tests exist for non-primary host environments
- product truth clearly distinguishes:
  - what works on Windows-primary hosts
  - what works cross-platform
  - what remains intentionally secondary

## Explicit Non-Scope for V0.2

Not in explicit `v0.2` scope:

- forms support beyond ordinary imported COM library support
- forms designer parity
- full VBA IDE parity
- non-Windows COM parity as a primary delivery target

Forms support is intentionally excluded from this workset rather than silently
deferred.

## Proposed Execution Epics

- `bd-bqm8` umbrella epic: `v02` V0.2 scope roundout execution
- `bd-bqm8.1` rollout/support bead for the `v0.2` execution graph (`v02.1`)
- `bd-bqm8.2` compat-slot excision (`v02.2`)
  - `bd-bqm8.2.1` audit and roll out compat-slot excision child beads
  - `bd-bqm8.2.2` remove or externalize VM/JIT core compat-slot surfaces
  - `bd-bqm8.2.3` migrate host/CLI/debugger observation off core slot
    projection
  - `bd-bqm8.2.4` externalize COM/HAL legacy compatibility bridges
  - `bd-bqm8.2.5` migrate tests, conformance notes, and product docs away from
    slot-shaped execution truth
  - `bd-bqm8.2.6` run the final excision checklist for `v02.2`
- `bd-bqm8.3` late-bound `IDispatch` parity clarification and delivery (`v02.3`)
  - `bd-bqm8.3.1` audit and roll out late-bound `IDispatch` child beads
  - `bd-bqm8.3.2` publish the supported/unsupported late-bound matrix
  - `bd-bqm8.3.3` deliver in-scope metadata-backed member/default/named-arg
    behavior
  - `bd-bqm8.3.4` extend controlled COM and host VM/JIT evidence for supported
    rows
  - `bd-bqm8.3.5` run the final late-bound `IDispatch` parity checklist
- `bd-bqm8.4` date-string parsing/coercion completion (`v02.4`)
  - `bd-bqm8.4.1` audit and roll out date-string parsing child beads
  - `bd-bqm8.4.2` publish accepted grammar and locale policy
  - `bd-bqm8.4.3` implement accepted parser/coercion gaps
  - `bd-bqm8.4.4` refresh VM/JIT/host/conformance evidence
  - `bd-bqm8.4.5` run the final date-string parsing checklist
- `bd-bqm8.5` runtime representation/layout investigation and architecture decision (`v02.5`)
- `bd-bqm8.6` VM/JIT hardening and security pass (`v02.6`)
- `bd-bqm8.7` Excel + Access/JET COM interop corpus buildout (`v02.7`)
- `bd-bqm8.8` language-service roundout for OxIde + LSP + VS Code (`v02.8`)
- `bd-bqm8.9` non-primary host validation breadth (`v02.9`)
- `bd-bqm8.10` native compilation path selection and scaffold (`v02.10`)
- `bd-bqm8.11` performance scaffold and VBA comparison harness (`v02.11`)

Current dependency truth:

- `bd-bqm8.2` depends on `bd-bqm8.1`
- `bd-bqm8.3` depends on `bd-bqm8.1` and `bd-bqm8.2`
- `bd-bqm8.4` depends on `bd-bqm8.1` and `bd-bqm8.2`
- `bd-bqm8.5` depends on `bd-bqm8.1` and `bd-bqm8.2`
- `bd-bqm8.6` depends on `bd-bqm8.1`, `bd-bqm8.2`, and `bd-bqm8.5`
- `bd-bqm8.7` depends on `bd-bqm8.1`, `bd-bqm8.3`, and `bd-bqm8.5`
- `bd-bqm8.8` depends on `bd-bqm8.1`
- `bd-bqm8.9` depends on `bd-bqm8.1`
- `bd-bqm8.10` depends on `bd-bqm8.1` and `bd-bqm8.5`
- `bd-bqm8.11` depends on `bd-bqm8.1` and `bd-bqm8.2`

Rollout sync, 2026-04-27:

- the dependency graph remains valid after value-model migration closure
- `bd-bqm8.1` is complete as a rollout/support bead
- the next ready delivery bead is `bd-bqm8.2`
- `bd-bqm8.5` remains downstream of `bd-bqm8.2`, but its role is now a
  V0.2 doctrine reconciliation over the completed value-model migration
  artifacts, not a second full representation migration

`v02.2` rollout, 2026-04-27:

- `bd-bqm8.2.1` published the compat-slot excision surface map in
  [V02_COMPAT_SLOT_EXCISION_ROLLOUT_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_COMPAT_SLOT_EXCISION_ROLLOUT_2026-04-27.md)
- `bd-bqm8.2` remains in-progress; the rollout does not close the capability
  lane
- `bd-bqm8.2.2` is complete for VM/JIT core public snapshot and slot-token
  surfaces; evidence is recorded in
  [V02_VM_JIT_COMPAT_SLOT_CORE_PROGRESS_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_VM_JIT_COMPAT_SLOT_CORE_PROGRESS_2026-04-27.md)
- `bd-bqm8.2.3` is complete for host, CLI, debugger export, immediate, and
  project integration observation boundaries; evidence is recorded in
  [V02_HOST_OBSERVATION_COMPAT_PROGRESS_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_HOST_OBSERVATION_COMPAT_PROGRESS_2026-04-27.md)
- `bd-bqm8.2.4` is complete for COM/HAL compatibility bridge
  externalization; COM `ComValue`/slot-token/dynamic/event callback projection
  and standard HAL wrapper projection now delegate through explicit
  `oxvba_com::compat` and `oxvba_hal::compat` adapters, with evidence in
  [V02_COM_HAL_COMPAT_BRIDGE_PROGRESS_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_COM_HAL_COMPAT_BRIDGE_PROGRESS_2026-04-27.md)
- `bd-bqm8.2.5` is complete for test, conformance, and product-doc migration
  away from slot-shaped execution truth; project integration expectations are
  now `expect_compat_slots`, CLI `SLOTS:` output is documented as
  compatibility observation, and remaining slot references are explicit
  adapter/evidence references, with evidence in
  [V02_TEST_DOC_COMPAT_SLOT_MIGRATION_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_TEST_DOC_COMPAT_SLOT_MIGRATION_2026-04-27.md)
- `bd-bqm8.2.6` is complete for the final compat-slot excision checklist, with
  evidence in
  [V02_COMPAT_SLOT_FINAL_CHECKLIST_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_COMPAT_SLOT_FINAL_CHECKLIST_2026-04-27.md)
- `bd-bqm8.2` is complete; the next ready V0.2 delivery bead is `bd-bqm8.3`
  for late-bound `IDispatch` parity clarification and delivery

`v02.3` rollout, 2026-04-27:

- `bd-bqm8.3.1` published the late-bound `IDispatch` supported/unsupported
  scope split and child delivery graph in
  [V02_IDISPATCH_PARITY_ROLLOUT_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_IDISPATCH_PARITY_ROLLOUT_2026-04-27.md)
- `bd-bqm8.3` remains in-progress; the rollout does not close the capability
  lane
- `bd-bqm8.3.2` is complete for publishing the supported/unsupported
  late-bound `IDispatch` matrix, with evidence in
  [V02_IDISPATCH_SUPPORTED_MATRIX_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_IDISPATCH_SUPPORTED_MATRIX_2026-04-27.md)
- `bd-bqm8.3.3` is complete for in-scope metadata-backed
  member/default/named-argument behavior, with evidence in
  [V02_IDISPATCH_METADATA_BACKED_BEHAVIOR_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_IDISPATCH_METADATA_BACKED_BEHAVIOR_2026-04-27.md)
- `bd-bqm8.3.4` is complete for controlled COM plus VM/JIT/host evidence for
  the supported rows, with evidence in
  [V02_IDISPATCH_CONTROLLED_COM_VM_JIT_HOST_EVIDENCE_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_IDISPATCH_CONTROLLED_COM_VM_JIT_HOST_EVIDENCE_2026-04-27.md)
- `bd-bqm8.3.5` is complete for the final late-bound `IDispatch` parity
  checklist, with evidence in
  [V02_IDISPATCH_FINAL_CHECKLIST_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_IDISPATCH_FINAL_CHECKLIST_2026-04-27.md)
- `bd-bqm8.3` is complete for the bounded V0.2 late-bound `IDispatch` lane;
  unsupported rows remain explicit in the matrix and conformance docs
- `bd-bqm8.4.1` is complete for rolling out the date-string parsing/coercion
  child beads, with evidence in
  [V02_DATE_STRING_ROLLOUT_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_DATE_STRING_ROLLOUT_2026-04-27.md)
- `bd-bqm8.4.2` is complete for publishing the accepted grammar, locale policy,
  and unsupported ambiguity boundaries, with evidence in
  [V02_DATE_STRING_GRAMMAR_POLICY_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_DATE_STRING_GRAMMAR_POLICY_2026-04-27.md)
- `bd-bqm8.4.3` is complete for accepted parser/coercion gaps, with evidence
  in
  [V02_DATE_STRING_IMPLEMENTATION_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_DATE_STRING_IMPLEMENTATION_2026-04-27.md)
- `bd-bqm8.4.4` is complete for VM/JIT/host/conformance evidence refresh,
  with evidence in
  [V02_DATE_STRING_VM_JIT_HOST_CONFORMANCE_EVIDENCE_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_DATE_STRING_VM_JIT_HOST_CONFORMANCE_EVIDENCE_2026-04-27.md)
- `bd-bqm8.4.5` is complete for the final date-string parsing checklist,
  with evidence in
  [V02_DATE_STRING_FINAL_CHECKLIST_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_DATE_STRING_FINAL_CHECKLIST_2026-04-27.md)
- `bd-bqm8.4` is complete for the bounded V0.2 date-string parsing/coercion
  lane; accepted rows have VM/JIT/host/conformance evidence and unsupported
  ambiguity remains explicit in the grammar policy

`v02.5` rollout, 2026-04-27:

- `bd-bqm8.5.1` is complete for rolling out representation/layout doctrine
  child beads, with evidence in
  [V02_REPRESENTATION_LAYOUT_ROLLOUT_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_REPRESENTATION_LAYOUT_ROLLOUT_2026-04-27.md)
- `bd-bqm8.5.2` is complete for the V0.2 representation/layout doctrine
  decision, with evidence in
  [V02_REPRESENTATION_LAYOUT_DECISION_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_REPRESENTATION_LAYOUT_DECISION_2026-04-27.md)
  and the accepted doctrine in
  [OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md](/C:/Work/DnaCalc/OxVba/docs/spec/OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md)
- `bd-bqm8.5.3` is complete for boundary evidence scans and representation
  risk classification, with evidence in
  [V02_REPRESENTATION_LAYOUT_EVIDENCE_SCAN_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_REPRESENTATION_LAYOUT_EVIDENCE_SCAN_2026-04-27.md)
- `bd-bqm8.5.4` is complete for the final representation/layout doctrine
  checklist, with evidence in
  [V02_REPRESENTATION_LAYOUT_FINAL_CHECKLIST_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_REPRESENTATION_LAYOUT_FINAL_CHECKLIST_2026-04-27.md)
- `bd-bqm8.5` is complete for the V0.2 representation/layout doctrine lane;
  semantic OxVba runtime values remain canonical internally, OLE Automation
  wire layouts remain boundary representations, and downstream boundary risk
  work is owned by `bd-bqm8.6`, `bd-bqm8.7`, and `bd-bqm8.10`

`v02.6` rollout, 2026-04-27:

- `bd-bqm8.6.1` is complete for rolling out VM/JIT hardening child beads, with
  evidence in
  [V02_HARDENING_ROLLOUT_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_HARDENING_ROLLOUT_2026-04-27.md)
- `bd-bqm8.6` remains in-progress; the next ready matrix bead is
  `bd-bqm8.6.2`, covering the V0.2 VM/JIT hardening matrix and scan baseline

## Delivery Rules for This Workset

- support-only audit or doc beads do not close any capability epic here
- every epic above requires delivery beads with executable evidence
- architecture investigation work does not close until it leaves behind:
  - a decision,
  - its evidence,
  - and the next concrete migration/delivery path
- unsupported boundaries discovered during this workset must be recorded
  explicitly and tested where practical

## Initial Priority Order

1. `v02.2` compat-slot excision
2. `v02.3` late-bound `IDispatch` parity clarification and delivery
3. `v02.4` date-string parsing/coercion completion
4. `v02.5` runtime representation/layout investigation
5. `v02.6` VM/JIT hardening and security pass
6. `v02.7` Excel + Access/JET COM interop corpus
7. `v02.8` language-service roundout
8. `v02.9` non-primary host validation breadth
9. `v02.11` performance scaffolding against VBA
10. `v02.10` native compilation path selection and scaffold

That ordering is intentional:

- first remove misleading internal seams
- then fix the most user-visible behavioral/compliance holes
- then make the architecture decision that affects future runtime/interop cost
- then broaden confidence, host coverage, and product-facing platform surfaces

## Exit Condition

This workset is complete only when:

- each in-scope `v0.2` family above is either:
  - parity-complete for its declared `v0.2` target, or
  - explicitly narrowed with evidence and a new owning in-progress path
- no broad product claim in repo docs still overstates the actual `v0.2`
  shipped subset for these families
- the resulting `v0.2` target is product-legible rather than merely
  implementation-legible
