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
| Procedure runtime metadata | procedure ids, params, return slot, entry | `crates/oxvba-compiler/src/emit.rs`; `ProcedureRuntimeMetadata`; `ProcedureSignatureDescriptor`; `crates/oxvba-vm/src/interpreter.rs`; `crates/oxvba-host/src/engine.rs`; `VmExecutionPackage`; `VmPackageIdentityEvidence` | Procedure name, module, entry PC, line maps, param slots/types, return slot/type, slot role metadata, and first signature descriptor view exist. VM package identity evidence now records package origin, package digest, bytecode digest, procedure ids, entry PCs, and slot counts for package execution, and host source/project/bundle/callable VM paths can expose the same recorded identity; full signature descriptor ids and descriptor digests do not. | `metadata-missing` | compiler-emit / VM package | Add stable signature descriptor ids, signature digests, and VM evidence comparing call behavior to descriptors. |
| Slot declared type metadata | `SlotTypeDescriptor` | `ProcedureRuntimeSlotMetadata`; `ProcedureRuntimeSlotKind`; `ProcedureRuntimeMetadata.param_types`; compiler resolver declaration maps; `TempSlotAllocator`; `OxBundle` v7; `VmExecutionPackage::slot_type_descriptors`; `VmPackageIdentityEvidence`; `conformance/vm_package/identity_seed` | `SlotTypeDescriptor` rows are populated for parameters, locals, return slots, compiler-generated fixed-array element slots, and expression temporaries in current procedure metadata, serialized through new bundles, upgraded from v3/v4 bundles, exposed by VM package setup, and reported in VM evidence with per-procedure descriptor digests. VM-runnable fixtures assert primitive scalar, `String`/`BStr`, declared `Variant`, and current UDT field-alias descriptor tokens alongside value snapshots. Temporary declared types, canonical descriptor ids, and richer shape-specific facts remain incomplete or explicitly `Unknown`. | `metadata-missing` | compiler-emit / VM evidence | Add canonical descriptor ids and expand descriptor population to richer array/UDT/object/fixed-string shapes without changing VM slot storage. |
| Primitive carriers | scalar carrier/layout descriptors | `DeclareParamType`; resolver `BoundType`; runtime arithmetic/coercion helpers; VM `RuntimeSlot`/`Variant` snapshots; `SlotTypeDescriptor` carrier hints | Primitive declarations, helper behavior, and first package slot carrier hints for `Long`, `Double`, and `Boolean` are fixture-backed; full carrier layout descriptors and operator/coercion descriptor ids are not package-owned. | `metadata-missing` | compiler-resolve / runtime / VM package | Promote carrier hints into canonical carrier/layout descriptors with behavior evidence before typed execution consumes them. |
| Declared Variant | COM-compatible Variant carrier descriptor | `crates/oxvba-runtime/src/variant.rs`; `Variant`; `VarType`; COM projection in `oxvba-com`; `SlotTypeDescriptor` carrier hints | Runtime Variant carrier exists, snapshots use it, and declared Variant slots now have first package descriptor evidence as `VbaTypeId::Variant` with `RuntimeCarrierKind::Variant`; COM VARIANT layout/projection descriptors are not yet package-owned. | `metadata-missing` | runtime / VM package / COM | Bind declared `Variant` slots to COM VARIANT projection descriptors and descriptor ids. |
| Decimal | Variant subtype/runtime payload | `crates/oxvba-runtime/src/decimal.rs`; `Decimal96`; `Variant::from_decimal96`; COM Decimal projection | Decimal payload support exists as a Variant subtype; declared Decimal storage remains an extension/audit concern. | `implemented-runtime-only` | runtime / compiler-type-system | Prevent declared Decimal storage except extension-gated rows; keep Decimal evidence under Variant carrier rows. |
| Strings | `BStr`, fixed string descriptors | `crates/oxvba-runtime/src/bstr.rs`; string bytecode/helpers; `DeclareParamType::String`; `SlotTypeDescriptor` carrier hints | Runtime `BStr`, declared String params/locals, and first `RuntimeCarrierKind::BStr` package descriptor evidence exist; fixed-length String descriptors and package cleanup obligations are missing. | `metadata-missing` | runtime / compiler-emit / VM evidence | Add fixed/variable string descriptor and cleanup obligation evidence. |
| Arrays | array type, shape, bounds, storage kind | `crates/oxvba-runtime/src/safe_array.rs`; `RuntimeArrayElementType`; resolver array descriptors; VM array instructions | Runtime `SafeArray` and some compiler/runtime array descriptors exist; package shape/provenance/evidence is incomplete. | `metadata-missing` | runtime / compiler-resolve / VM evidence | Add shape, bounds, Option Base, element carrier, and VM evidence descriptors. |
| UDTs | nominal type, fields, copy/init/cleanup | resolver/project UDT metadata; VM UDT field/copy paths; type-system draft; `conformance/vm_package/identity_seed` | UDT behavior and compiler metadata are partial. VM-runnable seed fixtures now record the current flattened base-slot plus field-alias descriptor shape, including primitive and `String` field carriers, but nominal package descriptor, field carrier ids, fixed field descriptors, offsets, and cleanup maps are missing. | `metadata-missing` | compiler-resolve / VM / runtime | Define package UDT descriptor and VM copy/field evidence before JIT use. |
| Objects/classes/interfaces | object/class/interface descriptors | `ObjectRef`; `RuntimeClassDescriptor`; `RuntimeInterfaceDescriptor`; project dynamic object routes; COM typelib projection | Runtime object identity and project/COM routes exist; unified package descriptors for VBA classes, interfaces, WithEvents, As New, and imported COM are incomplete. | `metadata-missing` | runtime-object / compiler-project / COM | Unify `ObjectRef`, class, interface, and COM imported descriptors with VM evidence. |
| Procedure signatures | full `ProcedureSignatureDescriptor` | `ProcedureRuntimeMetadata::procedure_signature_descriptor`; `ParameterDescriptor`; `ProcedureSignature` and `VbaTypeDescriptor` in `project.rs`; `ProjectDynamicMemberRoute`; `VmExecutionPackage::procedure_signature_descriptors`; `VmPackageIdentityEvidence::signature_call_evidence` | Package signature descriptor view exists for procedure kind, parameter order/slot/type, parsed ByRef/ByVal mode, source mechanism when known, resolved runtime mechanism, Optional/default and Variant-missing policy, ParamArray element/bounds shape, return type/slot, property group, property value ByVal semantics, and class hidden-receiver/`Me` metadata. VM package evidence now records signature descriptor digests and seed call-behavior observations; canonical descriptor ids and descriptor-driven call execution remain incomplete. | `metadata-missing` | compiler-project / compiler-emit / VM evidence | Add canonical signature ids where needed, then route VM call binding through package facts only after call-site descriptors are complete enough for the selected behavior lane. |
| Expression classification | `ExpressionSemanticsDescriptor` | resolver `BoundExpr`; compiler emit lowering; VM behavior | Expression categories drive current lowering, but they are not emitted as package descriptors. | `metadata-missing` | compiler-resolve / package | Add package descriptor for value, variable, property, function, member, and default-member shapes. |
| Let/Set coercion | `CoercionDescriptor` | runtime coercion helpers; assignment intent bytecode; resolver/VM behavior | Coercion behavior exists in helpers and interpreter paths; table rows and descriptor ids are not package metadata. | `metadata-missing` | runtime-coerce / compiler-emit / VM | Extract table and bind helper/evidence rows to descriptor ids. |
| Operators | `OperatorSemanticsDescriptor` | bytecode arithmetic/string/compare instructions; runtime arithmetic helpers; VM semantics | Operator behavior exists in instructions/helpers, but package operator table and helper mapping are missing. | `metadata-missing` | runtime-arithmetic / VM / compiler-bytecode | Build operator table and helper mapping before direct typed paths claim support. |
| Assignment/property | property accessor and value-param descriptors | `ProjectMemberCallDescriptor`; `ProjectDynamicMemberRoute`; assignment intent bytecode; VM behavior | Property routes and assignment intent exist for selected paths; full accessor groups, default member, and value-param descriptors are missing. | `metadata-missing` | compiler-project / VM object binding | Audit `Get`/`Let`/`Set`, default member, `Set`, and property value ByVal semantics. |
| Call sites | `CallSiteDescriptor` / `ArgumentBindingDescriptor` | compiler call lowering; `ProcedureRuntimeMetadata::call_sites`; `VmExecutionPackage::call_site_descriptors`; `VmPackageIdentityEvidence::call_site_evidence`; `ProjectDynamicParamRoute`; VM call binding | Seed package call-site rows and VM evidence exist for top-level project procedure/default-member fallback calls, including target kind, named/positional/omitted/ParamArray source shape, ByRef alias/writeback, ByRef expression temp/no-writeback, ByVal copy, Optional default, Optional `Variant` missing-policy metadata, empty/non-empty ParamArray packs, fixed-array materialization, property value ByVal classification, default-member fallback policy, and return copyout. Current evidence also shows ByVal declared-`Double` call entry and Optional `Variant` missing behavior are not yet VM-compatible. Expression call-site rows beyond current direct lowering, external Declare/COM coverage, canonical descriptor ids, descriptor-driven VM call binding, and true call-entry coercion/Optional-missing runtime behavior remain incomplete. | `metadata-missing`; `VM-limitation` | compiler-emit / VM call binding | Expand coverage to remaining expression calls, external/COM calls, and default-member policy details; fix or explicitly defer call-entry coercion and Optional-missing runtime behavior; then route selected VM call binding through package facts under VMR-06. |
| Error routing | error maps and resume targets | VM error fields; bytecode error instructions/patches; `ErrorFrame` in `interpreter.rs` | Runtime error state and resume behavior exist; package-level error/resume maps and descriptor evidence are missing. | `implemented-runtime-only` | VM / compiler-emit | Add package error/resume map and VM evidence schema. |
| Cleanup/lifetime | cleanup obligation map | runtime carriers; VM branch/return/error paths; COM/native marshaling helpers | Cleanup behavior exists in scattered runtime and boundary paths; package lifecycle/cleanup descriptors are missing. | `metadata-missing` | runtime / VM / COM-native | Add slot lifecycle, cleanup, error-exit, and deopt descriptors. |
| COM projection | COM descriptor set | `crates/oxvba-com`; `ProjectDynamicObjectRoute`; type-library projection in `project.rs`; VM COM bridge paths | Windows COM bridge and typelib/project routes exist; package descriptors are not yet the unified semantic source for all COM late/early/event paths. | `interop-limitation` | COM / compiler-project / VM host | Project from semantic descriptors, not raw wire types; capture VM evidence for TB06/TB07. |
| Native Declare | native ABI descriptor | `ExternalCallDescriptor`; `ExternalCallWriteback`; HAL `DynLinkDescriptorView`; host-backed native runner | Native descriptors and scalar/writeback lanes exist; general Automation `Variant`/`SAFEARRAY` declared-parameter ABI remains incomplete. | `interop-limitation` | compiler-bytecode / HAL / VM native | Audit scalar, BSTR, Variant, SAFEARRAY, and ByRef coverage and split residual ABI gaps. |
| Exported callable | inbound/outbound ABI descriptor | wrapper/export metadata; native export/XLL docs; tracer seed | Export metadata exists for wrapper/add-in lanes; VM/package inbound/outbound callable projection descriptor is not first-class. | `interop-limitation` | wrapper/export / host / package | Define package-level inbound projection, cleanup, writeback, and error return policy. |
| Host capability policy | host requirement descriptors | HAL `HostPolicy`; `CapabilityId`; host services; deterministic unsupported diagnostics | Host policy exists at runtime; digestable package capability requirements are missing. | `implemented-runtime-only` | HAL / host / package | Add digestable host capability requirements and unsupported diagnostics to package evidence. |
| Evidence schema | VM/JIT/package evidence | `scripts/run-jit-v2-tracer-fixtures.ps1`; `crates/oxvba-host/tests/jit_v2_tracer_vm_seed.rs`; `conformance/vm_package/identity_seed`; VM snapshot helpers; `crates/oxvba-host/src/engine.rs`; `VmPackageIdentityEvidence` | VM seed fixtures, retained snapshots, package/procedure/bytecode identity evidence, host package-identity snapshot/session evidence, first slot descriptor evidence with per-procedure digests, descriptor-token assertions for primitive/String/Variant/current-UDT-field slots, signature/call observations, and call-site descriptor evidence exist. Lifecycle, interop, host-policy observations, and broader expression-call descriptor coverage are not yet emitted. | `test-shortcoming` | VM evidence / conformance | Record lifecycle observations, boundary observations, host-policy diagnostics, and remaining expression-call descriptor coverage in VM evidence. |

## VMR-04 Call Fixture Gap Classification

These rows classify the call-fixture mismatches and uncovered call-shape gaps
before any VMR-06 behavior-affecting call-binding change may consume the
descriptors. A row marked as a gap is not a compatibility claim; it is a
decision about the next owner and evidence required before changing behavior.

| Evidence anchor | Current descriptor/evidence | Current VM behavior | Classification | Required before behavior change | Owner / follow-up |
|---|---|---|---|---|---|
| `VMR01_TYPED_FUNCTION`; `VMR04_CALL_ARGUMENT_BINDING`; `VMR04_BYREF_EXPRESSION_FORMS` ByRef literal/expression arguments | `CallSiteDescriptor` records `binding=byrefalias`/`writeback-required` for direct variables and `binding=byrefexpressiontemp`/`no-writeback-temp` for statement-level parenthesized force-ByVal, arithmetic expression, literal, and function-result source forms; signature evidence records `gap:byref-copyback-not-observed` for temporary forms. | The compiler lowers direct variable arguments as caller aliases with writeback. It lowers the currently VM-runnable non-variable forms into the callee parameter slot and emits no caller writeback. Exact typed ByRef variable mismatches are rejected by compiler diagnostics. | `oracle-required` for Office/MS-VBAL confirmation of each source form; `test-shortcoming` for property/default-member result forms not yet VM-runnable in the seed. This is not treated as a VM bug unless the spec/oracle says a covered source form should reject or route differently. | Use `VMR04_BYREF_EXPRESSION_FORMS` as the positive VM baseline for variable alias, parenthesized force-ByVal, expression, literal, and function-result behavior. Add Office-backed oracle evidence for property/default-member results and any source forms whose parse shape cannot be represented by current VM fixtures before broad descriptor-driven call binding consumes them. | `bd-iave.6.4` for the current VM-runnable baseline; object/default-member residuals remain with `bd-iave.7.3`/`bd-iave.7.4`; VMR-06 only after the oracle decision. |
| `VMR04_CALL_ARGUMENT_BINDING` `TakeDouble(ByVal value As Double)` called with a `Long` variable | Signature and call-site descriptors know the parameter declared type is `Double` and the binding kind is `ByValCopy`; no call-site `CoercionDescriptor` is package-owned yet. | The callee observes `VarType(value)=2` at entry and only later arithmetic produces an `f64:4.5` result. Runtime scalar carriers and `coerce_to(Long, Double)` exist, so the failure is not the primitive carrier itself. | `VM-limitation`; `metadata-missing` for descriptor-owned call-entry coercion facts. | Add a call-entry Let-coercion descriptor row and a VM fixture whose callee observes `VarType(value)=5` for the selected compatible case; route the selected call path through descriptor-backed coercion or explicitly defer it. | VMR-06 call binding (`bd-iave.9.1`/`bd-iave.9.2`) plus coercion rows (`bd-iave.8.1`). |
| `VMR03_SIGNATURE_CALLS`; `VMR04_CALL_ARGUMENT_BINDING` explicit Optional `Long = 7` | Signature and call-site descriptors record `optional-default=i32-7`; VM evidence records `optional-default-i32-observed`. | The callee receives the explicit default value and return copyout is observed. | No mismatch for the scoped fixture. Broader optional-default types remain `metadata-missing` until coercion/default-value descriptors are richer. | Keep this as the positive control while broadening default-value rows beyond `i32`. | Semantic table rows (`bd-iave.8`) before broad default-value behavior changes. |
| `VMR04_CALL_ARGUMENT_BINDING` `Optional ByVal value As Variant` omitted | Signature descriptor records `VariantMissingError448` with `PreserveMissingArgumentState`; call-site evidence records `optional-default=variant-missing-error-448`. | Current lowering emits the ordinary optional default path and the callee observes `VarType(value)=2`, not a preserved missing-argument state. | `VM-limitation`; `runtime-limitation` until a first-class missing-argument value state and `IsMissing`/introspection evidence exist; `metadata-missing` for the executable package value-state descriptor. | Define the missing-argument value state, confirm `IsMissing`/`VarType`/error behavior against MS-VBAL or Office oracle, then route omitted Optional `Variant` call binding through that state instead of `LoadConstI32 0`. | VMR-06 call binding (`bd-iave.9.1`/`bd-iave.9.2`) plus value-state/coercion rows (`bd-iave.8.1`). |
| `VMR03_SIGNATURE_CALLS`; `VMR04_CALL_ARGUMENT_BINDING` empty and non-empty `ParamArray` | Signature and call-site descriptors record `ParamArray` shape and element counts; VM evidence records pack observations and empty/non-empty upper-bound snapshots. | The scoped fixtures run and observe empty upper bound `-1` and two-element upper bound `1`. | No mismatch for the scoped fixture. Element coercion, named-argument rejection, lifetime, and non-Variant element projection remain `metadata-missing` / `test-shortcoming`. | Add rows/fixtures for element coercion, omitted elements, named `ParamArray` rejection, cleanup/lifetime, and non-Variant boundary projection before broad call binding claims. | `bd-iave.6.5`; VMR-05 descriptors, semantic table rows (`bd-iave.8`), or JIT-readiness gates (`bd-iave.10`) depending on array, lifecycle, or interop surface. |
| Named arguments, duplicate mappings, omitted required parameters, wrong argument counts | The target descriptor shape can represent argument source names and omitted values, but the current seed fixtures do not exercise the diagnostic paths and the compiler lowering path still performs partial direct mapping. | No VM-runnable package fixture proves the 448/449/450 diagnostic split for these source forms. | `test-shortcoming`; `metadata-missing`; possible `VM-limitation` only after executable fixtures prove the VM cannot route the required diagnostic. | Add VM-runnable call-binding diagnostic fixtures before selecting any descriptor-driven call behavior broader than the positive VMR-04 subset. | `bd-iave.6.5` before VMR-06 broad call binding. |
| Property/default-member, COM, native Declare, and exported-callable call-site shapes | Property value ByVal has seed evidence; COM/native/export descriptors are not unified call-site package facts for the VM package fixtures. | Current external and object paths run through specialized project/COM/native routes rather than a complete shared call-site descriptor contract. | `metadata-missing`; `test-shortcoming`; `interop-limitation` for COM/native/export boundaries. | Add VM/host evidence that preserves selector/default-member/named-arg/projection/writeback descriptors at the same package layer before JIT or native planning consumes them. | VMR-05 object evidence (`bd-iave.7.3`/`.7.4`) and JIT-readiness gates (`bd-iave.10.3`). |

## Tracer Bullet Package Fact Readiness

`VM seed status` records what can run under the current VM. `Package fact gap
kinds` records why the tracer is not yet executable-JIT-ready. The tracer
matrix at
[`../validation/JIT_V2_TRACER_BULLET_MATRIX_V1.csv`](../validation/JIT_V2_TRACER_BULLET_MATRIX_V1.csv)
must carry the same gap labels.

| Tracer | VM seed status | Required package facts | Package fact gap kinds | Readiness classification |
|---|---|---|---|---|
| TB01 Primitive typed scalar loop | `vm-ready` | package/procedure identity; slot descriptors for `Long`, `Double`, and `Boolean`; primitive carrier/layout descriptors; operator/coercion rows; loop PC/source maps; descriptor evidence | `metadata-missing`; `test-shortcoming` | VM source and retained snapshots run, and package/procedure/bytecode identity plus primitive/String/Variant slot descriptor-token evidence now exists; operator/coercion descriptor ids and full carrier layout descriptors are not package-owned yet. |
| TB02 UDT struct field/copy | `vm-ready` | UDT descriptor id; nominal type identity; field order, offsets, and carriers; whole-copy semantics; field cleanup/deopt maps; descriptor evidence | `metadata-missing`; `test-shortcoming` | VM source and projected field snapshots run, and package seed fixtures record the current flattened field-alias slot descriptors, but nominal UDT descriptors, field offsets, cleanup obligations, and aggregate descriptor ids are not yet a package contract. |
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
| VMR-02 | Slot descriptor surface | `SlotTypeDescriptor`, declared type ids, roles, initial states, carrier hints | metadata loading plus snapshots | VM can load and expose descriptors for parameters, locals, return slots, compiler-generated fixed-array element slots, and temporaries while still executing existing slots, and VM evidence reports descriptor digests and rows. Package fixtures assert primitive scalar, `String`/`BStr`, declared `Variant`, and current UDT field-alias descriptor tokens with value snapshots. Richer nominal array/UDT/object shape descriptors remain next steps. |
| VMR-03 | Signature descriptor surface | `ProcedureSignatureDescriptor`, `ParameterDescriptor`, return descriptor | metadata/evidence first | Compiler package and VM package setup expose full call-relevant signature facts plus VM evidence comparing current seed `CallProc` lowering against signature metadata for ByVal, ByRef, Optional default, ParamArray, property value, and return copyout behavior. Descriptor-driven call execution remains deferred to VMR-04/VMR-06. |
| VMR-04 | Expression and call descriptor seeds | `CallSiteDescriptor`, `ArgumentBindingDescriptor`, expression category descriptors | metadata/evidence first | Call-site descriptor rows and VM evidence now cover top-level project call lowering for ByRef alias/writeback, ByRef expression temp, ByVal copy with a declared-`Double` coercion gap, Optional default, Optional `Variant` missing-policy metadata, empty/non-empty ParamArray, property value ByVal classification, and return copyout. Remaining VMR-04 work must classify the current call-entry coercion and Optional-missing runtime limitations plus any uncovered expression/COM/native call-site gaps before behavior changes. |
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
