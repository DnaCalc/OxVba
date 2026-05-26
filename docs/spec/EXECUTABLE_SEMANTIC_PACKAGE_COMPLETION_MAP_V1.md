# Executable Semantic Package Completion Map v1

Status: `working-draft`
Date: 2026-05-26
Scope owner: OxVBA compiler/VM/runtime/JIT/native-readiness
Primary package reference:
[`EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](EXECUTABLE_SEMANTIC_PACKAGE_V1.md)

## Purpose

Map each required executable semantic package fact to its current or target
home. This is the bridge from the full semantic model to implementation work in
bytecode, compiler metadata, `OxBundle`, VM setup, runtime helpers, COM/native
descriptors, and evidence.

This document deliberately absorbs the gap-matrix role so we do not create a
separate validation CSV until the rows are stable enough to need automation.

## Inputs

Semantic authorities:

- [`VBA_TYPE_SYSTEM_V1.md`](VBA_TYPE_SYSTEM_V1.md)
- [`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md)
- [`BYTECODE_VM_SEMANTIC_CONTRACT_V1.md`](BYTECODE_VM_SEMANTIC_CONTRACT_V1.md)
- [`VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md`](VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md)

Current implementation anchors:

- `crates/oxvba-compiler/src/bytecode.rs`
- `crates/oxvba-compiler/src/emit.rs`
- `crates/oxvba-compiler/src/project.rs`
- `crates/oxvba-compiler/src/resolve.rs`
- `crates/oxvba-runtime`
- `crates/oxvba-vm/src/interpreter.rs`
- `crates/oxvba-vm/src/semantics.rs`
- `crates/oxvba-com`
- `crates/oxvba-host/src/engine.rs`

## Gap Taxonomy

Use these exact labels when classifying rows:

- `implemented`: the fact exists in a durable metadata surface and is consumed
  where needed.
- `implemented-runtime-only`: runtime behavior exists, but the fact is not yet
  exposed as package metadata.
- `test-shortcoming`: behavior and metadata exist, but VM-runnable evidence is
  missing.
- `metadata-missing`: behavior may exist, but package metadata does not carry
  the fact.
- `VM-limitation`: the VM cannot execute the scoped behavior yet.
- `runtime-limitation`: the runtime carrier/helper cannot represent or execute
  the scoped behavior yet.
- `interop-limitation`: COM/native/export projection is missing or narrower
  than the semantic model.
- `oracle-required`: MS-VBAL is ambiguous or Office behavior must be observed
  before the row can close.
- `deferred-extension`: not VBA-compatible by default and only allowed behind
  an explicit extension gate.

## Completion Matrix Schema

Future machine-readable rows should use this shape:

```text
semantic_area
target_descriptor
current_location
current_support
gap_kind
VM_required
JIT_required
interop_required
test_anchor
spec_anchor
next_action
owner
```

## Initial Map

The `Current support` column names what is actually present today. The `Gap
kind` column uses the taxonomy above and should not be upgraded until the
package descriptor and VM evidence requirements are both satisfied.

| Semantic area | Target descriptor | Current location | Current support | Gap kind | Owner | Next action |
|---|---|---|---|---|---|---|
| Bytecode control stream | instruction stream, entry PCs, slot counts | `crates/oxvba-compiler/src/bytecode.rs`; `Bytecode`; `Instruction`; `slot_count`; `user_slot_count` | Durable bytecode stream and slot counts exist and VM executes them. | `implemented` | compiler-bytecode / VM | Document per-op semantic contract and keep opcode-family rows in sync. |
| Procedure runtime metadata | procedure ids, params, return slot, entry | `crates/oxvba-compiler/src/emit.rs`; `ProcedureRuntimeMetadata`; `crates/oxvba-vm/src/interpreter.rs`; `crates/oxvba-host/src/engine.rs`; `VmExecutionPackage`; `VmPackageIdentityEvidence` | Procedure name, module, entry PC, line maps, param slots/types, return slot/type, and slot role metadata exist. VM package identity evidence now records package origin, package digest, bytecode digest, procedure ids, entry PCs, and slot counts for package execution, and host source/project/bundle/callable VM paths can expose the same recorded identity; full signature descriptor ids and descriptor digests do not. | `metadata-missing` | compiler-emit / VM package | Audit against full `ProcedureSignatureDescriptor`; add stable signature/slot descriptor ids and digest evidence. |
| Slot declared type metadata | `SlotTypeDescriptor` | `ProcedureRuntimeSlotMetadata`; `ProcedureRuntimeSlotKind`; `ProcedureRuntimeMetadata.param_types`; compiler resolver declaration maps; `TempSlotAllocator`; `OxBundle` v4; `VmExecutionPackage::slot_type_descriptors`; `VmPackageIdentityEvidence` | `SlotTypeDescriptor` rows are populated for parameters, locals, return slots, compiler-generated fixed-array element slots, and expression temporaries in current procedure metadata, serialized through new bundles, upgraded from v3 bundles, exposed by VM package setup, and reported in VM evidence with per-procedure descriptor digests. Temporary declared types, canonical descriptor ids, and richer shape-specific facts remain incomplete or explicitly `Unknown`. | `metadata-missing` | compiler-emit / VM evidence | Add canonical descriptor ids and expand descriptor population to richer array/UDT/object/fixed-string shapes without changing VM slot storage. |
| Primitive carriers | scalar carrier/layout descriptors | `DeclareParamType`; resolver `BoundType`; runtime arithmetic/coercion helpers; VM `RuntimeSlot`/`Variant` snapshots | Primitive declarations and helper behavior exist, but package carrier/layout descriptors are not preserved per slot. | `metadata-missing` | compiler-resolve / runtime / VM package | Preserve declared primitive slot carriers through package metadata and evidence. |
| Declared Variant | COM-compatible Variant carrier descriptor | `crates/oxvba-runtime/src/variant.rs`; `Variant`; `VarType`; COM projection in `oxvba-com` | Runtime Variant carrier exists and snapshots use it; declared Variant slot metadata is not package-owned. | `implemented-runtime-only` | runtime / VM package / COM | Bind declared `Variant` slots to package carrier metadata and COM VARIANT projection descriptors. |
| Decimal | Variant subtype/runtime payload | `crates/oxvba-runtime/src/decimal.rs`; `Decimal96`; `Variant::from_decimal96`; COM Decimal projection | Decimal payload support exists as a Variant subtype; declared Decimal storage remains an extension/audit concern. | `implemented-runtime-only` | runtime / compiler-type-system | Prevent declared Decimal storage except extension-gated rows; keep Decimal evidence under Variant carrier rows. |
| Strings | `BStr`, fixed string descriptors | `crates/oxvba-runtime/src/bstr.rs`; string bytecode/helpers; `DeclareParamType::String` | Runtime `BStr` and declared String params exist; fixed-length String descriptors and package cleanup obligations are missing. | `metadata-missing` | runtime / compiler-emit / VM evidence | Add fixed/variable string descriptor and cleanup obligation evidence. |
| Arrays | array type, shape, bounds, storage kind | `crates/oxvba-runtime/src/safe_array.rs`; `RuntimeArrayElementType`; resolver array descriptors; VM array instructions | Runtime `SafeArray` and some compiler/runtime array descriptors exist; package shape/provenance/evidence is incomplete. | `metadata-missing` | runtime / compiler-resolve / VM evidence | Add shape, bounds, Option Base, element carrier, and VM evidence descriptors. |
| UDTs | nominal type, fields, copy/init/cleanup | resolver/project UDT metadata; VM UDT field/copy paths; type-system draft | UDT behavior and compiler metadata are partial; nominal package descriptor, field carrier ids, fixed field descriptors, and cleanup maps are missing. | `metadata-missing` | compiler-resolve / VM / runtime | Define package UDT descriptor and VM copy/field evidence before JIT use. |
| Objects/classes/interfaces | object/class/interface descriptors | `ObjectRef`; `RuntimeClassDescriptor`; `RuntimeInterfaceDescriptor`; project dynamic object routes; COM typelib projection | Runtime object identity and project/COM routes exist; unified package descriptors for VBA classes, interfaces, WithEvents, As New, and imported COM are incomplete. | `metadata-missing` | runtime-object / compiler-project / COM | Unify `ObjectRef`, class, interface, and COM imported descriptors with VM evidence. |
| Procedure signatures | full `ProcedureSignatureDescriptor` | `ProcedureRuntimeMetadata`; `ProcedureSignature` and `VbaTypeDescriptor` in `project.rs`; `ProjectDynamicMemberRoute` | Multiple signature surfaces exist, but no single package signature descriptor covers all procedures, properties, Optional, ParamArray, `Me`, and defaults. | `metadata-missing` | compiler-project / compiler-emit | Capture source/resolved mechanisms, optional/default, ParamArray, `Me`, and property value shape. |
| Expression classification | `ExpressionSemanticsDescriptor` | resolver `BoundExpr`; compiler emit lowering; VM behavior | Expression categories drive current lowering, but they are not emitted as package descriptors. | `metadata-missing` | compiler-resolve / package | Add package descriptor for value, variable, property, function, member, and default-member shapes. |
| Let/Set coercion | `CoercionDescriptor` | runtime coercion helpers; assignment intent bytecode; resolver/VM behavior | Coercion behavior exists in helpers and interpreter paths; table rows and descriptor ids are not package metadata. | `metadata-missing` | runtime-coerce / compiler-emit / VM | Extract table and bind helper/evidence rows to descriptor ids. |
| Operators | `OperatorSemanticsDescriptor` | bytecode arithmetic/string/compare instructions; runtime arithmetic helpers; VM semantics | Operator behavior exists in instructions/helpers, but package operator table and helper mapping are missing. | `metadata-missing` | runtime-arithmetic / VM / compiler-bytecode | Build operator table and helper mapping before direct typed paths claim support. |
| Assignment/property | property accessor and value-param descriptors | `ProjectMemberCallDescriptor`; `ProjectDynamicMemberRoute`; assignment intent bytecode; VM behavior | Property routes and assignment intent exist for selected paths; full accessor groups, default member, and value-param descriptors are missing. | `metadata-missing` | compiler-project / VM object binding | Audit `Get`/`Let`/`Set`, default member, `Set`, and property value ByVal semantics. |
| Call sites | `CallSiteDescriptor` | compiler call lowering; `ProcedureRuntimeMetadata`; `ProjectDynamicParamRoute`; VM call binding | Current calls execute, but package call-site rows for arg mapping, ByRef temp/alias, Optional, named args, and ParamArray are missing. | `metadata-missing` | compiler-emit / VM call binding | Capture argument mapping, ByRef alias/temp, optional/default, named args, and ParamArray. |
| Error routing | error maps and resume targets | VM error fields; bytecode error instructions/patches; `ErrorFrame` in `interpreter.rs` | Runtime error state and resume behavior exist; package-level error/resume maps and descriptor evidence are missing. | `implemented-runtime-only` | VM / compiler-emit | Add package error/resume map and VM evidence schema. |
| Cleanup/lifetime | cleanup obligation map | runtime carriers; VM branch/return/error paths; COM/native marshaling helpers | Cleanup behavior exists in scattered runtime and boundary paths; package lifecycle/cleanup descriptors are missing. | `metadata-missing` | runtime / VM / COM-native | Add slot lifecycle, cleanup, error-exit, and deopt descriptors. |
| COM projection | COM descriptor set | `crates/oxvba-com`; `ProjectDynamicObjectRoute`; type-library projection in `project.rs`; VM COM bridge paths | Windows COM bridge and typelib/project routes exist; package descriptors are not yet the unified semantic source for all COM late/early/event paths. | `interop-limitation` | COM / compiler-project / VM host | Project from semantic descriptors, not raw wire types; capture VM evidence for TB06/TB07. |
| Native Declare | native ABI descriptor | `ExternalCallDescriptor`; `ExternalCallWriteback`; HAL `DynLinkDescriptorView`; host-backed native runner | Native descriptors and scalar/writeback lanes exist; general Automation `Variant`/`SAFEARRAY` declared-parameter ABI remains incomplete. | `interop-limitation` | compiler-bytecode / HAL / VM native | Audit scalar, BSTR, Variant, SAFEARRAY, and ByRef coverage and split residual ABI gaps. |
| Exported callable | inbound/outbound ABI descriptor | wrapper/export metadata; native export/XLL docs; tracer seed | Export metadata exists for wrapper/add-in lanes; VM/package inbound/outbound callable projection descriptor is not first-class. | `interop-limitation` | wrapper/export / host / package | Define package-level inbound projection, cleanup, writeback, and error return policy. |
| Host capability policy | host requirement descriptors | HAL `HostPolicy`; `CapabilityId`; host services; deterministic unsupported diagnostics | Host policy exists at runtime; digestable package capability requirements are missing. | `implemented-runtime-only` | HAL / host / package | Add digestable host capability requirements and unsupported diagnostics to package evidence. |
| Evidence schema | VM/JIT/package evidence | `scripts/run-jit-v2-tracer-fixtures.ps1`; `crates/oxvba-host/tests/jit_v2_tracer_vm_seed.rs`; `conformance/vm_package/identity_seed`; VM snapshot helpers; `crates/oxvba-host/src/engine.rs`; `VmPackageIdentityEvidence` | VM seed fixtures, retained snapshots, package/procedure/bytecode identity evidence, host package-identity snapshot/session evidence, and first slot descriptor evidence with per-procedure digests exist; expression/call descriptor usage, lifecycle, interop, and host-policy observations are not yet emitted. | `test-shortcoming` | VM evidence / conformance | Record expression/call descriptor usage, lifecycle observations, boundary observations, and host-policy diagnostics in VM evidence. |

## Tracer Bullet Package Fact Readiness

`VM seed status` records what can run under the current VM. `Package fact gap
kinds` records why the tracer is not yet executable-JIT-ready. The tracer
matrix at
[`../validation/JIT_V2_TRACER_BULLET_MATRIX_V1.csv`](../validation/JIT_V2_TRACER_BULLET_MATRIX_V1.csv)
must carry the same gap labels.

| Tracer | VM seed status | Required package facts | Package fact gap kinds | Readiness classification |
|---|---|---|---|---|
| TB01 Primitive typed scalar loop | `vm-ready` | package/procedure identity; slot descriptors for `Long`, `Double`, and `Boolean`; primitive carrier/layout descriptors; operator/coercion rows; loop PC/source maps; descriptor evidence | `metadata-missing`; `test-shortcoming` | VM source and retained snapshots run, and package/procedure/bytecode identity evidence now exists; declared primitive carrier facts, operator/coercion descriptor ids, and descriptor evidence are not package-owned yet. |
| TB02 UDT struct field/copy | `vm-ready` | UDT descriptor id; nominal type identity; field order, offsets, and carriers; whole-copy semantics; field cleanup/deopt maps; descriptor evidence | `metadata-missing`; `test-shortcoming` | VM source and projected field snapshots run, but UDT field carriers, offsets, cleanup obligations, and descriptor digests are not yet a package contract. |
| TB03 Error routing Resume Next | `vm-ready` | error/resume maps; resume target PC; failing helper/error descriptor; `Err` state evidence; deopt snapshot fields | `implemented-runtime-only`; `metadata-missing`; `test-shortcoming` | VM error behavior exists, but package-level error maps, helper failure descriptors, and evidence fields are missing. |
| TB04 BSTR lifetime | `vm-ready` | declared string slot descriptors; BSTR carrier/layout facts; concat/Len helper descriptors; branch/failure/deopt cleanup maps; lifetime counters | `metadata-missing`; `test-shortcoming` | VM string behavior runs, but fixed/variable string descriptor detail, cleanup obligations, and lifetime evidence are not package-owned. |
| TB05 SAFEARRAY For Each and bounds | `vm-ready-bounds-followup` | array shape/bounds descriptors; Option Base provenance; element carrier/lifetime maps; bounds-error evidence; SAFEARRAY ownership evidence | `metadata-missing`; `test-shortcoming` | VM positive subset runs for store/index/For Each and bounds metadata. Runtime bounds-error evidence and package array descriptors remain required before closure. |
| TB06 Late-bound COM Resume Next | `vm-ready-hosted` | COM object descriptor; selector/default-member/named-arg descriptors; HRESULT/EXCEPINFO/ArgErr projection; `ObjectRef` identity; host capability requirement | `interop-limitation`; `metadata-missing`; `test-shortcoming` | Hosted VM seed runs against controlled COM, but COM package descriptors and boundary evidence are not unified enough for executable JIT entry. |
| TB07 Early-bound COM typelib | `vm-ready-hosted` | typelib/reference descriptor; imported class/interface/member identity; dispatch-vtable strategy; argument/return projection; `ObjectRef` identity evidence | `interop-limitation`; `metadata-missing`; `test-shortcoming` | Hosted project seed runs with the OxVba typelib reference, but early-bound COM descriptors and strategy evidence are not yet package-owned. |
| TB08 Native Declare shared ABI | `vm-ready-native-hosted` | native ABI descriptor digest; scalar/BSTR/Variant/SAFEARRAY parameter projection; ByRef writeback policy; cleanup buffers; host dynamic-link policy | `interop-limitation`; `metadata-missing`; `test-shortcoming` | Current VM/native seed covers the implemented host-backed subset. General Automation `Variant` and `SAFEARRAY` declared-parameter ABI support remains a real interop limitation. |
| TB09 Exported callable projection | `vm-ready-export-followup` | inbound ABI projection descriptor; return projection; ByRef writeback policy; cleanup/error return policy; unsupported-shape diagnostics | `interop-limitation`; `metadata-missing`; `test-shortcoming` | Internal callable seed runs, but external inbound/outbound export projection is not a first-class VM/package descriptor yet. |

## VM Rework Readiness Slices

The first VM rework should move from evidence and metadata toward behavior
only after the descriptor surfaces are visible and fixture-backed. These slices
are intentionally ordered so a broad storage rewrite is not the first step.

| Slice | Goal | Descriptor families | VM change type | Stop condition |
|---|---|---|---|---|
| VMR-01 | Package identity and procedure metadata | package digest, procedure id, bytecode digest, entry PC, slot counts | metadata/evidence only | VM run evidence records package and procedure identity without changing behavior. |
| VMR-02 | Slot descriptor surface | `SlotTypeDescriptor`, declared type ids, roles, initial states, carrier hints | metadata loading plus snapshots | VM can load and expose descriptors for parameters, locals, return slots, compiler-generated fixed-array element slots, and temporaries while still executing existing slots, and VM evidence reports descriptor digests and rows. Richer shape-specific fixtures remain next steps. |
| VMR-03 | Signature descriptor surface | `ProcedureSignatureDescriptor`, `ParameterDescriptor`, return descriptor | metadata/evidence first | VM evidence can compare current call behavior against signature metadata. |
| VMR-04 | Expression and call descriptor seeds | `CallSiteDescriptor`, `ArgumentBindingDescriptor`, expression category descriptors | fixture-backed behavior audit | Fixtures classify call/argument gaps before behavior changes. |
| VMR-05 | Array, UDT, object descriptor seeds | `ArrayShapeDescriptor`, `UdtTypeDescriptor`, object/class/interface descriptors | metadata/evidence first | VM evidence captures shapes, field metadata, and object identity where current VM can execute. |
| VMR-06 | Behavior-affecting metadata consumption | selected call, array, coercion, and cleanup descriptors | targeted VM behavior changes | Each behavior change has a VM fixture and a completion-map gap classification. |

## Outstanding Ambiguities Before VM Rework

The first VM batch may proceed because the P0 ownership questions below are
either decided or explicitly tracked. These decisions permit metadata/evidence
work only; behavior-affecting VM consumption still waits for the later VMR-06
gate and fixture evidence.

| Ambiguity | Disposition | Rule for the next VM batch | Residual owner / follow-up |
|---|---|---|---|
| Descriptor id ownership | Decided | The executable semantic package owns canonical descriptor ids and descriptor digests. Compiler/resolver/emit code may contribute source facts, and `OxBundle` may serialize them, but VM and JIT consumers do not allocate semantic ids. Provisional in-memory ids are allowed only as evidence fields until canonical package assembly exists. | VMR-01 records package/procedure/bytecode identity; VMR-02 introduces descriptor views with stable digest inputs. |
| Carrier hint authority | Decided | First-pass carrier hints are observational evidence derived from declared type metadata. They must not drive slot storage, helper selection, typed lowering, or compatibility claims until a behavior-affecting VMR-06 bead promotes a specific descriptor use with fixture evidence. | VMR-02 captures declared type and carrier hint evidence; VMR-06 owns any future execution use. |
| Temporary slot modeling | Decided | Compiler-generated slots must receive `SlotTypeDescriptor` rows with role `Temporary` or `CompilerGenerated`, stable slot index, synthetic name when no source name exists, declared type if known, initial state, carrier hint, and cleanup obligation. User-visible slots remain bounded by current `user_slot_count` snapshot rules. | VMR-02 descriptor population beads define the first synthetic naming and cleanup fields. |
| In-memory package identity | Decided | A VM run that does not load a persisted `OxBundle` still records an in-memory package identity: package origin, bytecode digest, procedure identity, entry PC, slot counts, and descriptor digest set. The digest is evidence/cache correlation, not a public persistence format. | VMR-01 implements identity evidence for direct VM and host paths. |
| Partial resolver output | Tracked | Partial or unknown semantic facts must be represented as explicit unknown/unsupported descriptor states with deterministic diagnostics. The VM may continue current behavior for metadata-only evidence, but JIT/native entry gates must treat unknown descriptor facts as unsupported rather than rediscovering them. | VMR-04 and VMR-05 split call-site, object/member, array, UDT, and COM descriptor unknowns as they become executable fixtures. |
| Compatibility versus current behavior | Decided | Current VM behavior is executable truth for regression and differential evidence, but it is not automatically VBA compatibility truth. Any behavior-affecting metadata consumption must cite a spec/oracle/project decision when changing behavior; otherwise the package evidence records current behavior with the appropriate gap label. | Semantic table beads and future behavior beads must carry the authority row or oracle gate before closing parity language. |

No P0 ambiguity remains for the metadata/evidence-only first VM batch. The
tracked P1 details above are blockers only for the specific descriptor family
or behavior-affecting bead that consumes them.

## First Rework Boundary

The first VM rework batch is metadata/evidence only. It may add package views,
descriptor structs, descriptor digests, VM evidence fields, and fixtures that
show current behavior beside the target descriptors. It must not change runtime
slot storage, helper semantics, COM/native behavior, error routing, or
`oxvba-jit` status.

## Closure Rule

A row is complete only when:

- the descriptor exists in package metadata or a versioned descriptor referenced
  by the package;
- the VM consumes it or evidence proves it is not execution-affecting for the
  scoped row;
- VM-runnable fixtures cover the behavior or the row is explicitly oracle-only;
- JIT/native planning can reference the descriptor without rediscovering the
  semantic fact from source, bytecode pattern matching, or ambient runtime
  state.
