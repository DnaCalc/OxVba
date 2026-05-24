# Workset: Host Project Callable Reflection And Wrapper Generation Rework

Date: 2026-05-24
Owner: Codex
Status: complete
Bead root: `bd-hjys`
Related current epic: `bd-sg5h` — Host program design and UDF rework
Supersedes/refines: `docs/worksets/WORKSET_2026-05-10_HOST_PROGRAM_DESIGN_AND_UDF_REWORK.md`

## Purpose

Refactor the current host-UDF shape into a neutral host/project callable
reflection model plus generic build-time wrapper generation infrastructure.

The core correction is that OxVba should not hard-code a specialized "UDF"
concept into compiler, runtime, or generic build target layers. OxVba should let
an embedding process or generated wrapper decide what a public VBA procedure
means in that host: worksheet UDF, command, macro, service endpoint, CLI command,
DLL export, XLL registration entry, or another host-defined callable shape.

This workset defines and implements two related but separate hosting modes:

1. **In-process host embedding**: a process initializes a `VbaHost`, loads a VBA
   project from text/blob/file/bundle, reflects modules/procedures/signatures,
   applies its own host policy, and invokes procedures through typed, fast,
   diagnosable calls.
2. **Build-time wrapper generation**: a build profile reflects a VBA project and
   generates wrapper artifacts such as an introspection/call CLI executable,
   native-library exports, COM wrappers, and future XLL registration glue. XLL is
   a special instance of generic wrapper generation, not the foundation.

## First-Pass Development / No-Compatibility Rule

This workset is a first-pass boundary correction. The target end state is a
clean, legacy-free, compatibility-free codebase and doc set for this topic.

Binding rules:

- Delete old-shape `HostUdf*`, UDF-specific, adapter, shim, bridge, option, and
  compatibility code that is deprecated by this rework.
- Do not preserve legacy API names as wrappers unless a later user instruction
  explicitly requests a compatibility layer.
- Do not carry deprecated terminology in final docs except in audit/evidence
  sections that explain what was removed.
- Use git history for archeology; do not keep old code paths for archeology.
- New tests should assert the absence of deprecated public surfaces where
  practical, not adapter equivalence.

## Non-Goals

- Do not implement formula binding, formula name precedence, or worksheet recalc
  semantics in OxVba. Those remain OxFml/OxFunc/host responsibilities.
- Do not claim Excel/XLL parity in this workset. XLL support may consume the new
  wrapper-generation substrate later.
- Do not make compiler metadata decide which procedures are UDFs.
- Do not introduce a second DnaOneCalc-local comprehensive function registry.
- Do not add compatibility shims for the old host-UDF API shape.

## Boundary Principles

### Compiler boundary

The compiler owns neutral VBA facts only:

- project/module/procedure identity,
- module kind,
- visibility,
- procedure kind (`Sub`, `Function`, properties),
- parameter names, optional/default facts, ByVal/ByRef where known, and type
  facts where known,
- return type where known,
- source ranges/fingerprints,
- explicit source/project annotations such as category/help text when they are
  present in source or project metadata.

The compiler must not own:

- UDF admission,
- worksheet-visible naming policy,
- volatility/dependency policy,
- side-effect policy,
- thread-safety claims,
- host registry identity,
- Excel/XLL registration behavior.

### Runtime boundary

The runtime owns prepared execution and reflection-backed invocation primitives:

- prepare a compiled project into a runtime session,
- resolve a callable by stable callable ID or module/procedure route,
- validate arity and typed argument conversions for supported lanes,
- invoke with a generic host call context,
- return typed values or structured diagnostics.

The runtime must not decide that a call is a worksheet UDF. It may carry generic
context such as caller/provenance, locale, cancellation, and host metadata when a
host provides it.

### In-process host boundary

The `VbaHost` API owns host-facing project lifecycle and reflection:

```text
myVbaHost = new VbaHost(options)
myProject = myVbaHost.LoadProject(texts | streams | blob | file paths | bundle)
myMembers = myProject.GetModulesAndPublicProcedureSignatures()
myHostPolicyDecidesWhatIsAUdf(myMembers)
myPreparedProject.InvokeTyped(callable_id, context, args)
```

An embedding host may interpret public functions as UDFs, commands, services, or
not callable at all. OxVba supplies facts and invocation; the host supplies
policy.

### Build-time wrapper-generation boundary

Build targets consume the same neutral reflection model and produce host-specific
artifacts. A wrapper generator owns:

- callable selection,
- glue code generation,
- argument/result conversion lanes,
- host-specific registration/export metadata,
- host-specific diagnostics and packaging.

Examples:

- an introspection-printer/reflection-caller CLI executable,
- a wrapped native library with specific native exports,
- a COM server wrapper,
- a future XLL wrapper.

## Target API Shape

Names below are directional; final names should be settled during API design.

### In-process host

```rust
let host = VbaHost::new(VbaHostOptions::default());
let project = host.load_project(ProjectSource::from_texts(modules))?;
let reflection = project.reflect();

for module in reflection.modules() {
    for proc in module.public_procedures() {
        println!("{}::{} {:?}", module.name(), proc.name(), proc.signature());
    }
}

let prepared = project.prepare()?;
let callable = reflection.find_function("Main", "AddThem")?;
let result = prepared.invoke_typed(
    callable.id(),
    HostCallContext::new().with_caller("dnaonecalc:formula:Sheet1!A1"),
    &[TypedValue::Double(1.25), TypedValue::Double(2.5)],
)?;
```

### Neutral reflection descriptors

`ProjectReflection` should include at least:

- `ProjectIdentity`: name, source fingerprint, load fingerprint, optional bundle
  identity.
- `ModuleDescriptor`: module ID, source name, module kind, visibility flags,
  source fingerprint.
- `ProcedureDescriptor`: stable callable ID, module ID, name, kind, visibility,
  signature, source span/fingerprint, explicit metadata annotations.
- `ProcedureSignature`: parameters, return type, optional/default/ByRef facts,
  type text and normalized type enum where known.
- `CallableCapability`: neutral capability facts only, such as whether the
  current runtime can invoke the procedure and which conversion lanes are known.

Host-specific policy should be separate, for example:

- `HostCallableAdmission` owned by an embedding host,
- `WrapperCallableSelection` owned by a build wrapper generator,
- `UdfRegistrationRequest` owned by OxFunc, not OxVba.

### Generic host call context

Replace `HostUdfCallContext` with a neutral shape such as:

```rust
HostCallContext {
    caller: Option<HostCaller>,
    locale_id: Option<u32>,
    cancellation_token: Option<HostCancellationToken>,
    metadata: BTreeMap<String, HostContextValue>,
}
```

`HostCaller` should be provenance-oriented, not Excel-specific. Excel-like
callers can encode sheet/cell facts in a host-owned projection.

### Typed invocation

Typed invocation should be callable without UDF naming:

- `invoke_variant(callable_id, context, &[Variant]) -> InvocationResult`
- `invoke_typed(callable_id, context, &[TypedValue]) -> TypedInvocationResult`
- `signature_for(callable_id) -> ProcedureSignature`
- `validate_call(signature, args) -> CallValidation`

The first implementation may retain a narrow typed subset, but it should be
named as a conversion lane, not as a UDF feature.

## Build-Time Wrapper Generation Scope

This workset includes generic wrapper-generation infrastructure sufficient to
prove that build targets are consumers of reflection, not owners of compiler
policy.

### Wrapper generation package

Introduce or refactor toward a package/module that exposes:

- `ProjectReflectionInput`: compiled project, bundle, or loaded project.
- `CallableSelectionPlan`: select procedures by module/name/kind/visibility and
  host-specific annotations.
- `WrapperGenerationPlan`: output kind, callable selection, conversion lanes,
  diagnostics policy, and generated source/artifact contract.
- `GeneratedWrapper`: generated Rust/C/VBA/source snippets, manifest, and build
  metadata.
- `ArgumentParserPlan`: small generated parser for command-line or host-specific
  typed arguments.

### Required wrapper example: introspection-printer/reflection-caller EXE

Add a generated command-line executable wrapper example that embeds or loads the
compiled project and supports:

```text
my-wrapper.exe list
my-wrapper.exe describe Main.AddThem
my-wrapper.exe call Main.AddThem --double 1.25 --double 2.5
```

Acceptance requirements:

- `list` prints module/procedure inventory from reflected descriptors, not a
  hard-coded list in the wrapper.
- `describe` prints signature facts from descriptors.
- `call` resolves the callable by descriptor identity, parses typed CLI arguments
  through generated parser glue, invokes the runtime typed call path, and prints
  the typed result.
- Negative cases cover unknown procedure, arity mismatch, unsupported type, and
  runtime diagnostic propagation.

### WrappedNativeLibrary profile

Retain native exports as a build profile over generic reflection:

- explicit export selection is host/build policy,
- generated native thunks consume neutral callable descriptors,
- type marshaling is a conversion lane selected by the profile,
- no generic compiler/runtime code should label these as UDFs.

### COM and future XLL profiles

COM server and XLL wrapper work should consume the same reflection/wrapper
substrate:

- COM chooses class/interface/event wrapper plans.
- XLL chooses Excel registration metadata and formula-call glue.
- Both must avoid reparsing source or inventing metadata when bundle descriptor
  inventory is available.

XLL-specific implementation is out of scope except for ensuring the substrate can
represent a future XLL generation plan.

## Removal Of Current Host-UDF Code

Current old-shape surfaces to remove or replace directly:

- Delete `HostUdfCatalog` in favor of a neutral callable/reflection catalog.
- Delete `HostUdfFunctionDescriptor` in favor of `ProcedureDescriptor` plus
  optional host-owned policy examples outside compiler/runtime.
- Delete `HostUdfCallContext` in favor of `HostCallContext`.
- Delete `invoke_host_udf_with_variants` in favor of neutral
  `invoke_callable_with_variants`.
- Delete `host_udf_typed_signature` / `invoke_host_udf_typed` in favor of neutral
  typed signature/invoke lanes.
- Replace `BundleHostCallDescriptor` with neutral `BundleCallableDescriptor` or
  equivalent descriptor-inventory entry.

No compatibility adapter is required or desired. Any temporary bridge used
inside a single commit series must be removed before the owning bead closes.

## Implementation Phases

### Phase 0 — Audit and boundary inventory

- Inventory all `HostUdf*`, `host-call`, `UDF`, `XLL`, and wrapper-generation
  references in compiler/runtime/host/build crates.
- Classify each item as neutral fact, host policy, wrapper policy, removal
  target, or misplaced code.
- Publish a removal/replacement table listing what is deleted, what is renamed,
  and what remains.

### Phase 1 — Neutral descriptor model

- Define neutral project/module/procedure/signature descriptors.
- Ensure compiler and bundle descriptor inventory carry only neutral facts.
- Preserve explicit source/project annotations but do not synthesize worksheet or
  registry policy.
- Delete old descriptor projections that duplicate or conflict with the neutral model.

### Phase 2 — In-process `VbaHost` project API

- Introduce the host lifecycle facade:
  - construct host with options,
  - load project from text/blob/file/bundle,
  - reflect project before/after prepare,
  - prepare runtime session,
  - expose diagnostics.
- Ensure hosts can choose their own callable policy without modifying compiler
  metadata.

### Phase 3 — Runtime reflection and typed invocation

- Implement neutral callable resolution.
- Replace discarded `RuntimeCallFrame` behavior with an execution path that can
  actually receive generic `HostCallContext` where runtime or host services need
  it.
- Provide typed and variant invocation lanes with structured validation errors.
- Keep first-tier typed support explicit and documented.

### Phase 4 — Bundle descriptor source of truth

- Prepared sessions from bundles should consume descriptor inventory when
  present.
- Older bundles without descriptor inventory should produce an explicit
  descriptor-unavailable state rather than silently inventing policy.
- Source-loaded projects and bundle-loaded projects should project equivalent
  neutral reflection facts when descriptor inventory is available.

### Phase 5 — Generic wrapper-generation infrastructure

- Add/refactor wrapper generation abstractions.
- Implement generated introspection-printer/reflection-caller EXE example.
- Refactor native export and relevant wrapper code to consume neutral reflection
  and wrapper plans.
- Ensure generated wrappers do not hard-code callable lists except as generated
  descriptor tables derived from reflection.

### Phase 6 — Host-defined UDF interpretation layer

- Provide an example/helper outside compiler/runtime that maps neutral public
  functions to a host-owned UDF admission set.
- Keep OxFunc W093 alignment through neutral DTO projection, but do not implement
  registry mutation or formula precedence in OxVba.
- Delete current UDF-named APIs and migrate tests to neutral names.

### Phase 7 — Validation matrix and evidence refresh

- Update PH-0011 and related hosting validation matrix entries.
- Publish evidence for each suite below.
- Document unsupported/future lanes: XLL runtime parity, worksheet recalc, array
  and error returns beyond the admitted subset, host-specific name precedence.

## Required Test Suites

### Suite A — Compiler neutral facts

Purpose: prove compiler output is host-neutral.

Rows:

- Public procedural Function produces neutral procedure descriptor with signature.
- Public procedural Sub produces neutral procedure descriptor but no UDF policy.
- Private procedure is represented according to reflection visibility rules but
  not admitted by public-callable helper.
- Class-module methods remain class procedures, not standalone host UDFs.
- Explicit source/project annotations are preserved as annotations only.
- No descriptor field named volatility, worksheet-visible policy, registry key,
  or thread-safety is synthesized by compiler metadata.

### Suite B — Bundle descriptor roundtrip

Purpose: prove persisted descriptor inventory is the packaged source of truth.

Rows:

- Source project reflection and bundle reflection match for module/procedure
  identity and signatures.
- Bundle-loaded prepared session uses descriptor inventory when present.
- Legacy/no-inventory bundle reports explicit descriptor-unavailable state.
- Descriptor fingerprint changes when signature/name/source facts change.
- Descriptor fingerprint does not change when host policy overlays change.

### Suite C — In-process host lifecycle

Purpose: prove the primary embedding flow.

Rows:

- `VbaHost::load_project` from in-memory module texts.
- `VbaHost::load_project` from file paths or loader-provided blobs.
- `VbaHost::load_bundle` from `.oxb` bytes.
- Reflect modules and public method signatures before prepare.
- Prepare session and invoke selected callable.
- Multiple loaded projects remain isolated.
- Diagnostics identify load, compile, prepare, validation, and runtime phases.

### Suite D — Runtime callable invocation

Purpose: prove neutral runtime reflection/call semantics.

Rows:

- Variant invocation of public Function by callable ID.
- Typed Double invocation first slice.
- Typed Long/String/Boolean planning rows: either implemented or explicit
  unsupported diagnostics.
- Arity mismatch diagnostic.
- Type mismatch diagnostic.
- Runtime error propagation diagnostic.
- Generic `HostCallContext` caller/locale/metadata reaches the execution path or
  documented host-service observation point.
- Cancellation token shape is represented or explicitly deferred.

### Suite E — Host-defined UDF policy example

Purpose: prove UDF is a host interpretation.

Rows:

- Example host filters neutral public Functions into a UDF admission list.
- Example host rejects Subs, private functions, class methods, and unsupported
  signatures through host-owned policy.
- Example host maps descriptors to OxFunc W093-shaped registration requests
  without registry mutation.
- Changing host policy changes admission output without changing compiler/bundle
  descriptors.

### Suite F — Generated introspection/reflection-caller EXE

Purpose: prove generic wrapper generation.

Rows:

- Generated EXE `list` prints reflected modules/procedures from descriptor data.
- Generated EXE `describe Module.Proc` prints signature facts.
- Generated EXE `call Module.Add --double 1.25 --double 2.5` returns expected
  typed result.
- Generated parser rejects unknown switches, wrong arity, and unsupported type.
- Generated wrapper source contains generated descriptor data or bundle lookup,
  not manually hard-coded project procedure lists.
- Rebuild after adding/removing a VBA function changes generated list output.

### Suite G — WrappedNativeLibrary over wrapper plans

Purpose: prove native library exports are wrapper policy over reflection.

Rows:

- Explicit export selection selects a public Function by descriptor identity.
- Generated native thunk invokes the same neutral callable runtime path.
- Non-selected public Functions are not exported.
- Unsupported signature produces build-time wrapper diagnostic.
- Native export evidence does not mention UDF or worksheet policy.

### Suite H — COM and future XLL substrate checks

Purpose: keep adjacent build targets aligned without claiming full XLL work.

Rows:

- COM wrapper generation consumes descriptor inventory and does not reparse
  source for callable facts when inventory is present.
- Future XLL plan can be represented as a wrapper-generation plan with callable
  selection, Excel registration metadata placeholders, and conversion lanes.
- XLL execution/Excel registration remains explicitly unsupported/deferred until
  its own workset.

### Suite I — DNA Calc host integration shape

Purpose: cover DnaOneCalc/OxIde-style consumption.

Rows:

- DnaOneCalc-style host loads an OxVba project, reflects callables, chooses an
  admitted function, and invokes it through typed host API.
- OxIde-style host can inspect project/module/procedure inventory without
  preparing execution.
- Host-owned wrapper around callables can be cached by descriptor fingerprint and
  invalidated on project/module/function changes.
- No DnaOneCalc-local function mirror or formula precedence decision is created
  in OxVba.

### Suite J — Legacy removal and clean surface

Purpose: prove the final code/doc surface is legacy-free for this topic.

Rows:

- Deprecated `HostUdf*` public APIs, old options, shims, bridges, and docs are
  removed rather than adapted.
- Existing PH-0011 tests are rewritten to neutral API names.
- Evidence docs state which old claims are superseded and removed.
- Searches for old-shape terminology pass with only allowed audit/evidence
  references.
- Governance and validation matrices point at the new neutral boundary.

## Evidence Artifacts

Expected evidence files:

- `docs/evidence/host_callable/BOUNDARY_AUDIT.md`
- `docs/evidence/host_callable/NEUTRAL_DESCRIPTOR_MODEL.md`
- `docs/evidence/host_callable/IN_PROCESS_HOST_API.md`
- `docs/evidence/host_callable/RUNTIME_TYPED_INVOCATION.md`
- `docs/evidence/host_callable/BUNDLE_DESCRIPTOR_TRUTH.md`
- `docs/evidence/host_callable/WRAPPER_GENERATION_EXE.md`
- `docs/evidence/host_callable/WRAPPED_NATIVE_LIBRARY_PROFILE.md`
- `docs/evidence/host_callable/DNA_CALC_HOST_CONSUMPTION.md`
- refreshed PH-0011 matrix artifact.

## Bead Rollout

Created child beads under `bd-hjys`:

1. `bd-hjys.1` � Audit host-UDF and wrapper boundary leakage.
2. `bd-hjys.2` � Design neutral callable descriptor and VbaHost API contract.
3. `bd-hjys.3` � Implement neutral project reflection descriptors.
4. `bd-hjys.4` � Make bundle descriptor inventory callable source of truth.
5. `bd-hjys.5` � Introduce VbaHost load reflect prepare facade.
6. `bd-hjys.6` � Implement neutral callable invocation and context delivery.
7. `bd-hjys.7` � Delete HostUdf APIs and migrate tests to neutral replacements.
8. `bd-hjys.8` � Add host-owned UDF policy example and W093 projection.
9. `bd-hjys.9` � Implement generic wrapper generation plan abstractions.
10. `bd-hjys.10` � Generate introspection-printer and reflection-caller EXE wrapper.
11. `bd-hjys.11` � Refactor WrappedNativeLibrary over wrapper plans.
12. `bd-hjys.12` � Align COM wrapper and future XLL substrate with wrapper plans.
13. `bd-hjys.13` � Add DNA Calc host consumption examples.
14. `bd-hjys.14` � Refresh PH-0011 matrix and hosting evidence.
15. `bd-hjys.15` � Terminal audit for host callable reflection rework.

Dependency shape:

- `bd-hjys.1` gates `bd-hjys.2`.
- `bd-hjys.2` gates descriptor/API implementation and wrapper-plan design.
- `bd-hjys.3` gates bundle truth, host facade, runtime invocation, and wrapper plans.
- `bd-hjys.5` gates runtime context delivery and DNA Calc host examples.
- `bd-hjys.6` gates legacy HostUdf removal, wrapper invocation examples, and native wrapper refactor.
- `bd-hjys.9` gates generated wrapper examples, native wrapper refactor, and COM/future-XLL substrate checks.
- `bd-hjys.14` depends on generated wrapper, native wrapper, COM/future-XLL, and DNA Calc evidence.
- `bd-hjys.15` depends on the PH-0011/evidence refresh.

## Terminal Condition

This workset is complete only when:

1. compiler and bundle metadata expose neutral callable facts without embedded UDF
   policy,
2. in-process hosts can load, reflect, prepare, and invoke projects through a
   documented `VbaHost`-style API,
3. runtime callable invocation uses neutral context and typed/variant conversion
   lanes,
4. bundle descriptor inventory is the packaged source of truth where present,
5. UDF behavior is demonstrated as host-owned policy outside compiler/runtime,
6. generic wrapper-generation infrastructure is evidenced by a generated
   introspection/reflection-caller EXE,
7. WrappedNativeLibrary and adjacent wrapper plans consume neutral descriptors,
8. XLL remains framed as a future special wrapper profile rather than current
   generic UDF infrastructure,
9. DNA Calc host consumption examples prove no duplicated local function mirror
   or formula-precedence ownership in OxVba, and
10. PH-0011 and related evidence docs reflect the new boundaries.
