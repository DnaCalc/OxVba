# JIT v2 Cranelift Planning Stage Workset

Status: `planned`
Date: 2026-05-26
Scope owner: OxVBA JIT/native-readiness

## Purpose

Produce the reviewed design package, tracer-bullet backlog, and acceptance gates
needed before any broad Cranelift-based JIT v2 implementation starts.

This is a planning-only workset. It must not activate Cranelift dependencies,
change `oxvba-jit` behavior, or introduce executable JIT code. `oxvba-jit`
remains an explicit not-implemented API boundary until this workset's
implementation-entry gate passes.

Supporting research:
[`../reviews/JIT_V2_CRANELIFT_RESEARCH_REVIEW_2026-05-26.md`](../reviews/JIT_V2_CRANELIFT_RESEARCH_REVIEW_2026-05-26.md).

Current planning artifacts:

- VBA type system:
  [`../spec/VBA_TYPE_SYSTEM_V1.md`](../spec/VBA_TYPE_SYSTEM_V1.md)
- VBA expression and call semantics:
  [`../spec/VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](../spec/VBA_EXPRESSION_CALL_SEMANTICS_V1.md)
- Executable semantic package:
  [`../spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](../spec/EXECUTABLE_SEMANTIC_PACKAGE_V1.md)
- Executable semantic package VM strengthening:
  [`WORKSET_2026-05-26_EXECUTABLE_SEMANTIC_PACKAGE_VM_STRENGTHENING.md`](WORKSET_2026-05-26_EXECUTABLE_SEMANTIC_PACKAGE_VM_STRENGTHENING.md)
- Typed VM metadata bundle completion:
  [`WORKSET_2026-05-27_TYPED_VM_METADATA_BUNDLE_COMPLETION.md`](WORKSET_2026-05-27_TYPED_VM_METADATA_BUNDLE_COMPLETION.md)
- Typed VM metadata bundle implementation-entry audit:
  [`../validation/TYPED_VM_METADATA_BUNDLE_IMPLEMENTATION_ENTRY_AUDIT_2026-05-28.md`](../validation/TYPED_VM_METADATA_BUNDLE_IMPLEMENTATION_ENTRY_AUDIT_2026-05-28.md)
- Strict package-only VM handoff audit:
  [`../validation/STRICT_PACKAGE_ONLY_VM_HANDOFF_AUDIT_2026-05-28.md`](../validation/STRICT_PACKAGE_ONLY_VM_HANDOFF_AUDIT_2026-05-28.md)
- Implementation-entry review:
  [`../reviews/JIT_V2_IMPLEMENTATION_ENTRY_REVIEW_2026-05-26.md`](../reviews/JIT_V2_IMPLEMENTATION_ENTRY_REVIEW_2026-05-26.md)
- VM/native capability review:
  [`../reviews/JIT_V2_VM_NATIVE_CAPABILITY_REVIEW_2026-05-26.md`](../reviews/JIT_V2_VM_NATIVE_CAPABILITY_REVIEW_2026-05-26.md)
- Implementation design:
  [`../spec/JIT_V2_IMPLEMENTATION_DESIGN_V1.md`](../spec/JIT_V2_IMPLEMENTATION_DESIGN_V1.md)
- ProcLoweringIr detail:
  [`../spec/JIT_V2_PROC_LOWERING_IR_V1.md`](../spec/JIT_V2_PROC_LOWERING_IR_V1.md)
- Differential harness:
  [`../spec/JIT_V2_DIFFERENTIAL_HARNESS_V1.md`](../spec/JIT_V2_DIFFERENTIAL_HARNESS_V1.md)
- Semantic contract and fact pack:
  [`../spec/JIT_V2_SEMANTIC_CONTRACT_AND_FACT_PACK_V1.md`](../spec/JIT_V2_SEMANTIC_CONTRACT_AND_FACT_PACK_V1.md)
- Helper ABI catalog:
  [`../spec/JIT_V2_HELPER_ABI_CATALOG_V1.md`](../spec/JIT_V2_HELPER_ABI_CATALOG_V1.md)
- Tracer-bullet test plan:
  [`../spec/JIT_V2_TRACER_BULLET_TEST_PLAN_V1.md`](../spec/JIT_V2_TRACER_BULLET_TEST_PLAN_V1.md)
- Tracer-bullet validation matrix:
  [`../validation/JIT_V2_TRACER_BULLET_MATRIX_V1.csv`](../validation/JIT_V2_TRACER_BULLET_MATRIX_V1.csv)
- Support matrix:
  [`../validation/JIT_V2_SUPPORT_MATRIX_V1.csv`](../validation/JIT_V2_SUPPORT_MATRIX_V1.csv)
- Initial fixture set:
  [`../../conformance/jit_v2/tracer_bullets/`](../../conformance/jit_v2/tracer_bullets/)
- Hosted VM seed test:
  [`../../crates/oxvba-host/tests/jit_v2_tracer_vm_seed.rs`](../../crates/oxvba-host/tests/jit_v2_tracer_vm_seed.rs)

## Current Truth Baseline

- Bytecode plus VM execution is the current executable semantic authority.
  The next evolution is a complete executable semantic package: bytecode plus
  declared type metadata, slot metadata, UDT/array/object descriptors,
  COM/native descriptors, error/source maps, helper requirements, and host
  capability requirements consumed by both VM and JIT.
- The current `OxBundle` is the seed artifact for that package, not yet the
  complete package contract.
- `oxvba-jit` is a disabled public boundary. Unsupported JIT requests must fail
  explicitly rather than silently falling back to VM execution.
- JIT v2 must preserve declared VBA type semantics. Primitive scalars,
  `BStr`/String, `ObjectRef`, `SafeArray`, UDT structs, and declared `Variant`
  values are distinct planning carriers. `Variant` remains mandatory where the
  source type or COM boundary requires the Windows VARIANT layout, but it is not
  the universal native value model.
- JIT v2 type metadata follows the authoritative VBA type-system reference.
  Decimal is modeled as a Variant subtype/runtime carrier, and object/class,
  interface, `WithEvents`, `As New`, and imported COM types are first-class
  descriptor-backed declared type categories.
- JIT v2 expression and call metadata follows the VBA expression/call
  semantics reference. Let/Set coercion, operator behavior, property access,
  Optional/ParamArray binding, and ByRef/ByVal aliasing/writeback are package
  facts, not JIT-side reconstruction.
- Current VM snapshots use retained `Variant` values as oracle evidence. That
  snapshot carrier is an observation/projection contract, not permission to
  build another Variant-only JIT.
- COM is first-class in JIT v2 planning. Late-bound COM, early-bound COM,
  HRESULT/EXCEPINFO projection, object identity, SAFEARRAY transport, and native
  Declare interop must be present in the first design slice.
- The first supported JIT target policy is Windows x64 only. Non-Windows and
  unsupported-target JIT requests remain deterministic unavailable until a
  separate target-support decision accepts them. The support matrix records
  target availability only; executable tracer entry remains governed by the
  package/VM evidence gates in
  `docs/validation/JIT_V2_TRACER_BULLET_MATRIX_V1.csv`.
- Cranelift is the selected backend, but Cranelift is only a code generator
  behind OxVba semantic contracts. VBA semantics, cleanup, error routing,
  COM/native descriptors, and deopt behavior belong to OxVba planning artifacts.
- The typed VM metadata bundle implementation-entry audit and the strict
  package-only VM handoff audit are the current package handoff gates. They
  permit support-scaffolding implementation to consume package facts, but any
  tracer row still carrying a package/VM/test/interop/oracle gap remains a
  deterministic reject/classify path until its named evidence gate passes.

## Execution Policy

1. Do not implement JIT execution under this workset.
2. Do not lower directly from bytecode to ad hoc CLIF. A real `ProcLoweringIr` or
   equivalent semantic contract is required first.
3. Do not create a parallel typed JIT path. Any semantic fact required for JIT
   lowering must be present in the executable semantic package or a versioned
   descriptor referenced by it, and must be visible to VM execution/evidence.
4. No silent fallback is allowed. Any fallback-like behavior must be an explicit
   deopt or slow-helper contract with VM/JIT snapshot equality tests.
5. No ambient helper symbol lookup is allowed in the design. Runtime helpers,
   COM/native thunks, and exported callable trampolines must flow through an
   audited helper ABI manifest.
6. Initial Cranelift memory access policy is conservative. Stronger flags such
   as `readonly`, `aligned`, `notrap`, `can_move`, or alias narrowing require a
   named proof for the exact carrier and host-boundary assumptions.
7. Cranelift verifier gates are required for every compiled function in future
   tests/debug lanes.
8. Textual CLIF and source/bytecode maps are diagnostic artifacts only. Semantic
   proof comes from VM/JIT differential evidence and runtime/interop assertions.
9. Debug mode may disable JIT or restrict it to a conservative profile until the
   debug/source mapping policy is explicitly accepted.
10. No Variant-universal frame model is allowed. `ProcLoweringIr` must carry
   declared slot/carrier kinds, including primitive and UDT layout metadata;
   `VariantComLayout` is reserved for declared `Variant`, dynamic coercion, and
   COM/native boundary projection.
11. JIT support scaffolding may start before the VM strengthening workset is
   complete, but executable tracer work is gated by package/VM evidence for the
   descriptor families it consumes. At minimum, package identity, procedure
   identity, bytecode digest, signature descriptors, and slot descriptors must
   be visible in VM evidence for the touched fixtures. TB01 additionally gates
   on declared primitive slot/carrier evidence; TB02 additionally gates on UDT
   descriptor and owning-field lifecycle evidence; TB03 additionally gates on
   package-owned error/resume maps and failing-helper evidence; TB04
   additionally gates on non-UDT BSTR layout, helper, cleanup, and lifetime
   evidence; TB05 additionally gates on array descriptor, SAFEARRAY bounds,
   element lifetime, and ownership evidence; TB06/TB07 additionally gate on
   COM boundary descriptors, object identity, error projection, selector
   metadata, and cleanup evidence; TB08 additionally gates on native ABI,
   writeback, cleanup, and error-policy evidence; and TB09 additionally gates
   on exported-callable ABI projection, cleanup/error policy, writeback, and
   unsupported-shape diagnostics before `ProcLoweringIr` consumes those facts.

## Planning Deliverables

- **JIT v2 fact pack:** inventory VM instruction semantics, bytecode metadata,
  declared type metadata, runtime carriers, error state, COM/native call paths,
  lifetime rules, UDT layout/copy rules, and existing conformance anchors.
- **VBA type-system alignment:** use `VBA_TYPE_SYSTEM_V1.md` as the reference
  for declared types, runtime value states, runtime carriers, object/class/
  interface/COM descriptors, and Decimal-as-Variant-subtype policy.
- **VBA expression/call alignment:** use
  `VBA_EXPRESSION_CALL_SEMANTICS_V1.md` as the reference for expression
  classification, Let/Set coercion, operator semantics, assignment/property
  behavior, Optional/ParamArray binding, and ByRef/ByVal call-site descriptors.
- **Executable semantic package inventory:** document which required semantic
  facts already live in `Bytecode`, `OxBundle`, procedure metadata, runtime
  descriptors, or VM behavior, and classify missing facts as package gaps,
  VM limitations, or test shortcomings.
- **JIT support matrix:** record target triple, OS, arch, Cranelift backend
  status, COM/native availability, executable-memory policy, and deterministic
  unavailable behavior.
- **JIT semantic contract:** define VM-equivalent behavior for typed slot state,
  declared `Variant`/COM VARIANT projection, UDT field layout/copy behavior,
  error state, object identity, BSTR/SAFEARRAY ownership, ByRef writeback, COM
  HRESULT/EXCEPINFO, host policy, cleanup, and diagnostics.
- **ProcLoweringIr proposal:** specify blocks, terminators, slot/value effects,
  helper calls, frame maps, cleanup edges, error/resume edges, COM/native
  descriptors, deopt points, and source/bytecode mapping.
- **ProcLoweringIr verifier plan:** define pre-CLIF checks for block termination,
  dominance, slot effects, cleanup stack balance, helper ABI references,
  safepoint/live-carrier maps, and source/bytecode maps.
- **Runtime helper ABI manifest:** catalog helper symbols, calling convention,
  params/returns, ownership transfer, may-allocate, may-run-host-code,
  may-reenter, may-set-Err, cleanup obligations, and versioning.
- **Interop-first design note:** align COM and native DLL/SO interop around
  shared ABI descriptors instead of separate mechanisms.
- **Deopt/snapshot contract:** define how compiled code reconstructs VM slot
  state, error state, cleanup state, ByRef writebacks, and host/COM boundary
  state.
- **VM/JIT differential harness design:** define how the same executable
  semantic package runs through VM and JIT with identical `HostServices`,
  policy, descriptors, and snapshot collection.
- **Tracer-bullet backlog:** define implementation probes and acceptance tests
  that expose the hardest semantic risks early.

The current planning package is the implementation-entry baseline for the first
JIT v2 support-scaffolding workset. The implementation-entry review records the
P0/P1 decisions and review gate results. This workset remains planning-only: it
is permission to start support-query, `ProcLoweringIr`, verifier, helper
manifest, and harness-unavailable scaffolding, not permission for executable
tracer work and not evidence that JIT execution exists.

## Execution Epics

| Order | Epic | Purpose | Close condition |
|---|---|---|---|
| 1 | Fact pack, package, and VM truth inventory | Establish the VM, bytecode, executable semantic package, declared type metadata, runtime carrier, UDT layout, error-state, lifetime, and conformance truth the JIT must preserve. | Fact pack names every scoped truth surface, identifies package gaps, and links residuals to tracer-specific gates. |
| 2 | Semantic contract and runtime helper ABI | Define VM-equivalent behavior and the helper ABI manifest used by all generated code. | Semantic contract and helper catalog are reviewed, versioned, and have no unowned P0 gaps. |
| 3 | `ProcLoweringIr` and Cranelift lowering design | Specify lowering from the executable semantic package into procedure-lowering IR, verifier, frame layout, safepoints, deopt metadata, and CLIF lowering rules. | `ProcLoweringIr` proposal and verifier plan are complete enough for tracer-bullet implementation without design invention. |
| 4 | COM/native interop-first design | Make late-bound COM, early-bound COM, native Declare, exported callable paths, object identity, and HRESULT/EXCEPINFO behavior first-class package descriptors. | COM/native design proves shared ABI descriptor use and names unsupported shapes deterministically. |
| 5 | Tracer-bullet backlog and acceptance tests | Turn the design into ordered probes with exact evidence requirements. | Every tracer bullet has fixtures, expected VM/JIT evidence, package metadata evidence, cleanup/error assertions, and fallback/deopt behavior defined. |
| 6 | Review gates and implementation-entry checklist | Run VM truth, package completeness, COM/native, Cranelift, and fresh-eyes reviews before code begins. | All P0/P1 questions are answered or explicitly blocked, and broad JIT implementation remains gated until acceptance artifacts are complete. |

## Tracer Bullets And Acceptance Gates

1. **Primitive typed scalar loop**
   - Compile a small arithmetic loop over declared `Long`, `Double`, and
     `Boolean` locals.
   - Acceptance: VM/JIT typed slot snapshots match after projection; primitive
     carrier layout is documented; VM package evidence includes the declared
     primitive slot/carrier facts consumed by `ProcLoweringIr`; helper
     fallback/deopt is specified; CLIF verifier is required; no unsafe memory
     flags are used for frame loads or stores; `Variant` is used only for
     declared `Variant` or VM snapshot materialization.
2. **UDT struct field/copy path**
   - Compile field assignment, whole-UDT copy, field update, and typed field
     arithmetic over a declared UDT.
   - Acceptance: VM/JIT UDT field snapshots match after projection; struct
     descriptor, field offsets, copy semantics, cleanup, and deopt
     materialization are documented; VM package evidence includes UDT
     descriptor and lifecycle facts consumed by `ProcLoweringIr`; no field is
     boxed as Variant unless the declared field type is `Variant`.
3. **Error-routing path**
   - Compile `On Error Resume Next`, `Err.Number`, and a failing helper call.
   - Acceptance: VM/JIT error state, resume target, and slot snapshots match;
     VM package evidence includes error/resume maps, failing-helper
     descriptors, resume target evidence, and `Err`-state snapshot fields
     consumed by `ProcLoweringIr`; slow-helper or deopt behavior is explicit.
4. **BSTR lifetime path**
   - Compile declared `String` assignment, concat, and `Len` over real `BStr`.
   - Acceptance: allocation, branch-exit cleanup, helper-failure cleanup, early
     return cleanup, and deopt cleanup are mapped and tested; VM package
     evidence includes declared String/BSTR layout facts, concat/Len helper
     descriptors, cleanup maps, and lifetime counters consumed by
     `ProcLoweringIr`.
5. **SAFEARRAY path**
   - Compile typed `Long` array store/index/`For Each` plus array literal
     metadata over real `SafeArray`.
   - Acceptance: bounds errors, element lifetime, live maps around helpers, and
     snapshot equality are proved; VM package evidence includes array shape
     descriptors, `Option Base` provenance, runtime SAFEARRAY bounds, element
     lifetime facts, and ownership policy consumed by `ProcLoweringIr`.
6. **Late-bound COM path**
   - Compile `CreateObject` plus `IDispatch::Invoke` with named/default member
     and failure under `Resume Next`.
   - Acceptance: HRESULT, EXCEPINFO, ArgErr, `Err` projection, object identity,
     and named/default dispatch metadata match VM/COM behavior; VM package
     evidence includes COM activation/dispatch descriptors, selector metadata,
     error projection fields, object identity observations, and cleanup facts
     consumed by `ProcLoweringIr`.
7. **Early-bound COM path**
   - Compile a known typelib-backed call using metadata-derived dispatch or
     vtable strategy.
   - Acceptance: descriptor identity, object identity, and dispatch-vs-vtable
     parity are proved, not only call success; VM package evidence includes
     typelib/imported member descriptors, dispatch-vtable strategy evidence,
     argument/return projection facts, object identity observations, and
     cleanup facts consumed by `ProcLoweringIr`.
8. **Native Declare path**
   - Compile a DLL/SO call with scalar, BSTR, Variant, SAFEARRAY, and ByRef
     writeback.
   - Acceptance: shared ABI descriptor reuse, writeback, cleanup on failure,
     and error policy are proved; VM package evidence includes native ABI
     descriptor digests, projection facts, ByRef writeback evidence,
     cleanup/buffer ownership evidence, and native error policy consumed by
     `ProcLoweringIr`.
9. **Exported callable path**
   - Expose a JIT-backed procedure through wrapped COM/native export shape.
   - Acceptance: inbound ABI projection, cleanup, error return policy, and
     unsupported-shape diagnostics are defined with no silent fallback; VM
     package evidence includes inbound/outbound projection descriptors, ByRef
     writeback policy, cleanup/error return policy, and unsupported-shape
     diagnostics consumed by `ProcLoweringIr`.

## Review And Implementation-Entry Gate

Required reviews:

1. **VM truth review:** validate each planned JIT behavior against the VM
   interpreter, bytecode metadata, and current runtime carrier model.
2. **Package completeness review:** validate that every semantic fact needed by
   the first tracer bullets is present in the executable semantic package or is
   tracked as a package/VM/test gap before JIT lowering uses it.
3. **COM/native review:** treat COM and native interop as first-class, including
   declared `Variant` as Windows VARIANT at boundaries, late-bound COM,
   early-bound COM, events, object identity, SAFEARRAY, HRESULT/EXCEPINFO,
   registration/export boundaries, and native Declare.
4. **Cranelift review:** confirm lowering strategy, calling conventions, helper
   imports, verifier use, relocation/module model, debug/deopt metadata,
   unsupported-target behavior, and memory-flag policy.
5. **Fresh-eyes review:** explicitly look for hidden alternate value models,
   JIT-only shortcuts, Variant-universal modeling, missing primitive/UDT typed
   lanes, missing cleanup, missing error edges, COM fallback leakage,
   unsupported host reentry, and unproved memory assumptions.

Implementation may start only when:

- all P0/P1 design questions are answered or documented as blockers;
- the fact pack, executable semantic package inventory, semantic contract,
  `ProcLoweringIr` proposal, helper ABI manifest, COM/native design note, and
  differential harness design are present;
- every tracer bullet has acceptance tests defined;
- executable tracer work is tied to the executable semantic package VM
  strengthening slices for the descriptor families it consumes;
- the review gates above are recorded;
- `oxvba-jit` still reports not implemented until the first implementation
  workset explicitly changes that contract with evidence.

## P0/P1 Design Decisions

- Initial compiled function shape:
  `extern "C" fn(vmctx: *mut JitVmContext, frame: *mut JitFrame) -> JitStatus`.
- Shared input layer: VM and JIT consume the same executable semantic package.
  The current `OxBundle` is the seed, and missing package facts are blockers for
  JIT lowering rather than material for backend-side rediscovery.
- First-slice frame model: declared typed slots are authoritative in
  `ProcLoweringIr`. Primitive scalars and UDT structs must have native carrier and
  layout metadata. VM-compatible retained `Variant` snapshots are materialized
  for evidence/deopt, and declared `Variant` slots use exact COM VARIANT layout.
- Helper ABI: versioned helper table, no ambient symbol lookup,
  pointer/descriptor arguments through `JitVmContext` and `JitFrame`, and no
  Rust unwinding across generated-code boundaries.
- Live-carrier model: explicit live-carrier, cleanup, ByRef, interop, and deopt
  maps are required for slice 1. Cranelift user stack maps are a later extension
  unless implementation review finds a concrete first-slice need.
- Debug policy: JIT remains disabled by default in debug sessions until a
  conservative debug profile is accepted.
- Diagnostics: unsupported target, disabled backend, debug-policy disabled,
  unsupported bytecode, deopt requested, helper fault, COM/native failure, and
  real JIT execution are distinct statuses/diagnostic rows.
- Target policy: Windows x64 is the only first accepted JIT target.
  Non-Windows and non-x64 requests are deterministic unavailable. Target
  availability does not bypass the tracer matrix package/VM evidence gates.
- COM/native policy: late COM, early COM, native Declare, and exported callable
  paths use shared descriptor-backed helpers before any specialization.

## Verification For This Planning Workset

File creation and future updates should run:

```text
./scripts/check-governance.ps1
./scripts/run-jit-v2-tracer-fixtures.ps1
```

Future implementation worksets must add targeted crate tests, Cranelift verifier
checks, VM/JIT differential tests, and COM/native fixture evidence before making
JIT execution claims.

## Terminal Gate

The planning terminal gate for opening the first JIT v2 support-scaffolding
implementation workset is passed by
[`../reviews/JIT_V2_IMPLEMENTATION_ENTRY_REVIEW_2026-05-26.md`](../reviews/JIT_V2_IMPLEMENTATION_ENTRY_REVIEW_2026-05-26.md).
Tracer-specific gates remain active: no tracer bullet is closed until its
VM/JIT differential evidence, verifier evidence, cleanup/error evidence, and
COM/native evidence where relevant have passed. Executable tracer work also
depends on the VM strengthening evidence for the package facts it consumes.
Completion of this planning workset is not a claim that JIT execution exists.
