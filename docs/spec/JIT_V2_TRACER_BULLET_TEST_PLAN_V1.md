# JIT v2 Tracer Bullet Test Plan v1

Status: `planning-test-design`
Date: 2026-05-26
Owning workset:
[`../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md`](../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md)
Implementation design:
[`JIT_V2_IMPLEMENTATION_DESIGN_V1.md`](JIT_V2_IMPLEMENTATION_DESIGN_V1.md)
Validation matrix:
[`../validation/JIT_V2_TRACER_BULLET_MATRIX_V1.csv`](../validation/JIT_V2_TRACER_BULLET_MATRIX_V1.csv)
Differential harness:
[`JIT_V2_DIFFERENTIAL_HARNESS_V1.md`](JIT_V2_DIFFERENTIAL_HARNESS_V1.md)
Executable semantic package:
[`EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](EXECUTABLE_SEMANTIC_PACKAGE_V1.md)
Expression/call semantics:
[`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md)

## Purpose

Define the initial tests that make JIT v2 implementation immediately
test-driven. The fixtures live outside the active basic-language conformance
runner until the JIT implementation workset adds harness support.

Fixture root:
`conformance/jit_v2/tracer_bullets/`

Planning-stage VM seed check:
`./scripts/run-jit-v2-tracer-fixtures.ps1`

Hosted VM seed check for COM/type-library/native fixtures:
`cargo test -p oxvba-host --test jit_v2_tracer_vm_seed -- --nocapture`

## Harness Contract

The future VM/JIT differential harness must run each tracer bullet with the same
executable semantic package, `HostServices`, host policy, and snapshot
collector.

For every fixture, collect:

- backend result status;
- package digest and required package metadata facts;
- declared carrier/layout evidence;
- expression/coercion/operator/call-site descriptor evidence when relevant;
- VM-compatible slot snapshot, currently materialized as retained `Variant`
  evidence;
- `Err` state snapshot;
- cleanup/lifetime counters when relevant;
- COM/native boundary diagnostics when relevant;
- JIT support diagnostic if unavailable;
- `ProcLoweringIr` verifier result;
- Cranelift verifier result once CLIF exists;
- optional textual CLIF artifact for diagnosis.

The harness passes only when the VM and JIT semantic evidence match for the
declared observable subset. CLIF text alone never proves correctness.

## VM Package Evidence Prerequisite

The VM seed fixtures prove what currently runs under the VM; they are not by
themselves permission for the JIT to rediscover missing facts. Before a tracer
fixture can become executable JIT work, the VM evidence for that fixture must
record the package/procedure identity, bytecode digest, and descriptor digests
for the slot, signature, expression/call, array/UDT/object, COM/native, cleanup,
or error facts the tracer consumes.

If the VM can run the source but cannot expose the required package facts, the
gap is `metadata-missing`. If the VM cannot run the source, classify the reason
as `test-shortcoming`, `VM-limitation`, `runtime-limitation`, or
`interop-limitation` before the tracer is treated as executable-JIT-ready.

## Fixture Manifest

The fixture manifest is:
[`../../conformance/jit_v2/tracer_bullets/manifest.csv`](../../conformance/jit_v2/tracer_bullets/manifest.csv).

Expected VM seed values:
[`../../conformance/jit_v2/tracer_bullets/expected_vm_values.csv`](../../conformance/jit_v2/tracer_bullets/expected_vm_values.csv).

Current fixture statuses:

- `vm-ready`: source is intended to run through the current VM and become a JIT
  differential fixture later.
- `vm-ready-bounds-followup`: source is VM-runnable for the SAFEARRAY store,
  index, `For Each`, and bounds-metadata subset, but a separate runtime
  bounds-error fixture is still required before that tracer bullet can close.
- `vm-ready-error-package-evidence`: source is VM-runnable and package
  evidence records the current error-routing/deopt descriptor subset, but
  broader error maps, active-handler quirks, and cleanup-stack execution remain
  required before that tracer bullet can close.
- `vm-ready-export-package-evidence`: source is VM-runnable for the internal
  callable seed and package evidence records the current exported-callable
  descriptor subset, but external ABI execution breadth is still required
  before that tracer bullet can close.
- `vm-ready-hosted`: source is VM-runnable when the test supplies required
  host setup, such as controlled COM or project/type-library metadata.
- `vm-ready-native-hosted`: source is VM-runnable through the current
  host-backed native dynamic-link lane; unsupported wider ABI shapes remain
  explicit tracer residuals.
- `export-boundary-planned`: source is not yet VM-runnable and is primarily for
  inbound/outbound export projection tests.

The tracer matrix separates VM seed status from package readiness:

- `current_status` is only the current VM seed/run shape.
- `package_fact_gap_kinds` carries the completion-map gap labels that block
  executable JIT entry.
- `package_fact_readiness` summarizes which required package facts are present
  only as VM behavior, missing metadata, missing evidence, or incomplete
  interop projection.

## Tracer Bullet Assertions

### TB01 Primitive Typed Scalar Loop

Fixture:
`conformance/jit_v2/tracer_bullets/tb01_primitive_scalar_loop.bas`

Package/VM evidence gate:

- VM package evidence must include package/procedure/bytecode identity and slot
  descriptor rows for the declared `Long`, `Double`, and `Boolean` slots before
  `ProcLoweringIr` may consume this tracer.
- The current package identity seed fixtures provide the first declared
  primitive slot/carrier evidence; canonical primitive carrier layout
  descriptors and operator/coercion descriptor ids remain blocking facts before
  executable JIT work may close TB01.

Required assertions:

- VM and JIT projected slot snapshots match.
- `ProcLoweringIr` records declared `Long`, `Double`, and `Boolean` carriers.
- `ProcLoweringIr` contains one loop header, one loop body, one exit block, and
  explicit branch terminators.
- Primitive direct ops, if used, are justified by declared carrier metadata and
  have helper/deopt paths for unsupported coercion or overflow behavior.
- Cranelift verifier passes.
- No strong memory flags are used for frame slot loads/stores.
- Declared primitive slots are not boxed as `Variant` in the JIT frame.

### TB02 UDT Struct Field/Copy

Fixture:
`conformance/jit_v2/tracer_bullets/tb02_udt_struct_fields.bas`

Package/VM evidence gate:

- VM package evidence must include nominal `UdtTypeDescriptor` rows, field
  order/carrier facts, field aliases, copy classification, and lifecycle
  evidence for owning fields before `ProcLoweringIr` may consume this tracer.
- The current VM package fixtures provide UDT descriptor evidence and selected
  BSTR-owning field lifecycle evidence. Field offsets/layout, explicit
  cleanup/deopt maps, and descriptor-driven UDT field/copy execution remain
  blocking facts before executable JIT work may close TB02.

Required assertions:

- VM and JIT projected field snapshots match.
- `ProcLoweringIr` records UDT descriptor id, field offsets, and field carrier
  kinds.
- Whole-UDT copy preserves source/destination independence.
- Field load/store operations are verifier-checked against descriptor bounds.
- Cleanup and deopt materialization cover every owning field.
- No UDT field is boxed as VARIANT unless the declared field type is `Variant`.

### TB03 Error Routing Resume Next

Fixture:
`conformance/jit_v2/tracer_bullets/tb03_error_resume_next.bas`

Package/VM evidence gate:

- VM package evidence must include error/resume maps, failing-helper
  descriptors, resume target evidence, and `Err`-state snapshot fields before
  `ProcLoweringIr` may consume this tracer.
- The current VM seed proves runtime behavior, and package evidence now records
  `ErrorRoutingDescriptor` rows for `On Error Resume Next` plus the division
  helper failure edge, including Err-state fields, runtime error 11,
  resume-next-consumable policy, and `DeoptSnapshotDescriptor` rows for helper
  safepoints with slot-map, live-carrier, error-state, cleanup-state, and
  ByRef-state obligations. Strict VM support accepts selected Err reset and
  call-frame safepoint rows, but rejects the Resume Next helper-failure shape
  through `ERROR-CLEANUP-DEOPT-UNSUPPORTED`. Broader Resume forms,
  active-handler reentry, caller unwinding, and explicit cleanup-stack execution
  remain blocking before executable TB03 can close.

Required assertions:

- Division failure routes through JIT error helper.
- Failing division uses declared primitive inputs, not an accidental
  Variant-only arithmetic path.
- `Err.Number` equals VM value after `On Error Resume Next`.
- Failed operation preserves slot state according to VM behavior.
- Resume target is next bytecode PC.
- Deopt/slow-helper path records error state before continuing.
- Current VM package evidence asserts the selected error/deopt descriptor
  tokens for the Resume Next division seed and strict rejection of non-selected
  error/deopt cleanup consumption.

### TB04 BSTR Lifetime

Fixture:
`conformance/jit_v2/tracer_bullets/tb04_bstr_lifetime_concat_len.bas`

Package/VM evidence gate:

- VM package evidence must include declared `String` slot descriptors, BSTR
  carrier/layout facts, concat/`Len` helper descriptors, and cleanup maps for
  return, branch, helper-failure, and deopt paths before `ProcLoweringIr` may
  consume this tracer.
- The current lifecycle seed table and selected UDT BSTR-owning lifecycle
  evidence are useful supporting evidence, but they do not close the non-UDT
  declared String/BSTR lifetime gate for TB04. Allocation/release counters and
  helper failure cleanup observations remain blocking.

Required assertions:

- String assignment, concat, and `Len` snapshots match.
- Source uses declared `String`/`Long` locals so the BSTR path is not reached
  through untyped Variant-only source.
- Every allocation has cleanup ownership.
- Branch exit and return paths drain cleanup obligations.
- Helper failure and deopt exits preserve or release BSTR ownership exactly
  once.

### TB05 SAFEARRAY

Fixture:
`conformance/jit_v2/tracer_bullets/tb05_safearray_foreach_bounds.bas`

Package/VM evidence gate:

- VM package evidence must include `ArrayShapeDescriptor` rows, `Option Base`
  provenance, runtime SAFEARRAY bounds evidence, element carrier/lifetime facts,
  and SAFEARRAY ownership policy before `ProcLoweringIr` may consume this
  tracer.
- The current package fixtures provide fixed/static and dynamic local array
  descriptors, dynamic `ReDim` runtime bounds, and selected rank-1
  fixed/static `LBound`/`UBound` descriptor-backed execution. Runtime
  bounds-error evidence, multi-rank coverage, VM lifecycle ownership
  observations, and COM/native SAFEARRAY projection remain blocking.

Required assertions:

- Typed `Long` array element stores and `For Each` snapshots match.
- Index reads and dynamic-array `LBound`/`UBound` snapshots match the VM seed
  fixture.
- Package array-shape evidence records fixed/static bounds, explicit
  `0 To 2` bounds, dynamic runtime SAFEARRAY bounds, `Option Base`, element
  type/carrier, and fixed-array base-slot allocation status.
- Package execution resolves rank-1 fixed/static `LBound`/`UBound` from
  `ArrayShapeDescriptor`; raw bytecode execution remains the baseline runtime
  error for the same unallocated fixed-array base slot.
- A follow-up runtime bounds-error fixture must be added before TB05 closes;
  the current active compiler/runtime does not yet expose that failure route
  through a VM-runnable standalone tracer shape.
- Multi-rank fixed/static bound evidence remains a follow-up before TB05
  closure.
- Element carrier lifetimes are recorded in safepoint live maps.
- SAFEARRAY descriptor and payload ownership remain runtime-owned.

### TB06 Late-Bound COM

Fixture:
`conformance/jit_v2/tracer_bullets/tb06_late_bound_com_resume_next.bas`

Package/VM evidence gate:

- VM package evidence must include COM activation/dispatch descriptors,
  named/default member selector evidence, HRESULT/EXCEPINFO/ArgErr projection
  fields, `ObjectRef` identity observations, and boundary cleanup facts before
  `ProcLoweringIr` may consume this tracer.
- The current hosted VM seed records `CreateObject`/dispatch instruction
  expectations, runtime selector source, `early_bound=false`, and
  runtime-owned HRESULT/EXCEPINFO/ArgErr classification, runtime result
  projection classification, and host dispatch cleanup classification.
  Strict VM support rejects this broader boundary execution through
  `BOUNDARY-CONSUMPTION-UNSUPPORTED`.
  Generalized package-owned boundary projection, object identity comparison,
  default-member facts, and concrete cleanup execution evidence remain
  blocking.

Required assertions:

- `CreateObject` and `IDispatch::Invoke` use descriptor-backed helpers.
- Named/default member metadata is captured in helper descriptor evidence.
- HRESULT, EXCEPINFO, ArgErr, and `Err` projection match VM/COM behavior.
- Object identity is retained as `ObjectRef`; no raw COM pointer becomes a VM
  slot value.
- Current VM seed is hosted because the controlled `OxVba.TestDispatch` object
  is provided by the Rust host test, not by standalone CLI ProgID registration.
- Current VM package evidence records hosted `CreateObject` and dispatch invoke
  instruction expectations, runtime selector source, `early_bound=false`, and
  runtime-owned HRESULT/EXCEPINFO/ArgErr classification, runtime
  Variant/Object result projection classification, and host dispatch cleanup
  classification, plus strict rejection of the non-selected COM boundary
  execution row. It does not yet prove generalized package-owned boundary
  projection or cleanup execution behavior.

### TB07 Early-Bound COM

Fixture:
`conformance/jit_v2/tracer_bullets/tb07_early_bound_com_typelib.bas`

Package/VM evidence gate:

- VM package evidence must include typelib/reference descriptors, imported COM
  class/interface/member descriptors, dispatch-vs-vtable strategy evidence,
  `ObjectRef` identity observations, and argument/return projection facts
  before `ProcLoweringIr` may consume this tracer.
- The current hosted VM seed records early-bound dispatch instruction
  expectations, selector policy, runtime-owned HRESULT/EXCEPINFO/ArgErr
  classification, argument/result projection classification, and cleanup
  classification. Strict VM support records the selected
  `VMR09-COM-DISPATCH-SELECTOR-001` row and rejects broader COM boundary
  execution through `BOUNDARY-CONSUMPTION-UNSUPPORTED`. The object/member seed
  table names early-bound dispatch plus vtable strategy obligations. Imported
  member descriptors, strategy digest evidence, object identity comparison, and
  typed projection execution facts remain blocking.

Required assertions:

- Typelib-backed descriptor identity is present.
- Dispatch/vtable strategy is explicit and evidence-backed.
- Object identity and return slots match VM behavior.
- Missing arg/failure variants later extend this fixture family before closure.
- Current VM seed is hosted because typed imported COM binding requires a
  project manifest with an `OxVba` type-library reference.
- Current VM package evidence records the early-bound dispatch instruction
  expectation, the selected COM selector consumption row, and boundary policy
  classifications. Imported COM class/interface/member descriptor identity and
  dispatch-vtable strategy remain package gaps.

### TB08 Native Declare

Fixture:
`conformance/jit_v2/tracer_bullets/tb08_native_declare_shared_abi.bas`

Package/VM evidence gate:

- VM package evidence must include native ABI descriptor digests, scalar/BSTR
  pointer/Variant cell/SAFEARRAY buffer projection facts, ByRef writeback
  evidence, cleanup-buffer ownership evidence, and native error policy before
  `ProcLoweringIr` may consume this tracer.
- The current hosted VM seed records `ExternalCallDescriptor` and native invoke
  facts for the supported subset, including DeclareParamType projection tokens,
  writeback projection classifications, cleanup policy, and native error
  policy. Strict VM support records the selected
  `VMR09-NATIVE-DESCRIPTOR-INVOKE-001` row and rejects broader native boundary
  execution through `BOUNDARY-CONSUMPTION-UNSUPPORTED`. Generic Automation
  `Variant` and SAFEARRAY declared-parameter ABI support plus broader
  cleanup/buffer ownership observations remain real VM/native limitations
  before executable TB08 can close.

Required assertions:

- Native Declare uses the shared ABI descriptor machinery.
- Current VM seed covers scalar native calls, BSTR/string pointer access,
  SAFEARRAY byte-buffer pointer access, Variant cell pointer exposure, and
  scalar ByRef writeback through the existing descriptor machinery.
- General Automation `Variant` and `SAFEARRAY` declared-parameter ABI support is
  a real current VM/native limitation and remains a future tracer-closure
  requirement.
- Current VM package evidence records `ExternalCallDescriptor` and native
  invoke facts for library/alias/name, return/parameter tokens, ByRef flags,
  argument slots, writeback slots, projection classifications, cleanup policy,
  native error policy for the supported seed subset, and strict rejection of
  broader native boundary execution.
- Writeback commits or cancels according to helper status.
- Cleanup runs for marshalled buffers on success, failure, and deopt.

### TB09 Exported Callable

Fixture:
`conformance/jit_v2/tracer_bullets/tb09_exported_callable_projection.bas`

Package/VM evidence gate:

- VM package evidence must include inbound ABI projection descriptors, ByRef
  writeback policy, return projection, cleanup/error return policy, and
  unsupported-shape diagnostics before `ProcLoweringIr` may consume this
  tracer.
- The current VM seed proves the internal callable behavior, and the project
  package seed records exported-callable descriptor evidence derived from
  `ExportInventory` plus callable signatures. Strict VM support records the
  selected `VMR09-EXPORTED-CALLABLE-DESCRIPTOR-001` row for package procedure
  invocation metadata. External ABI entry execution, typed/non-Variant
  unmanaged projection breadth, concrete cleanup/error oracle evidence, and
  unsupported-shape diagnostic fixtures remain blocking TB09 gaps.

Required assertions:

- Inbound ABI projection populates retained `Variant` slots.
- ByRef inbound arguments have explicit writeback policy.
- Return value projection and error return policy are deterministic.
- Unsupported inbound shapes report diagnostics with no silent VM fallback.
- Current VM package evidence records Variant-positional inbound projection,
  declared parameter types, ByRef export-boundary writeback policy,
  return-slot-to-Variant projection, cleanup/error return policy, and
  unsupported-shape policy for the descriptor-backed package seed, plus selected
  exported-callable descriptor consumption evidence.

## Acceptance Before First Implementation Workset

Before starting JIT execution implementation:

- this plan and the matrix are cross-linked from the workset;
- all fixture paths exist;
- every fixture has a declared current status and intended evidence;
- every fixture names the executable semantic package facts it needs and
  whether the current VM/package path already exposes them;
- the future harness result shape is specified;
- `oxvba-jit` still reports not implemented.
