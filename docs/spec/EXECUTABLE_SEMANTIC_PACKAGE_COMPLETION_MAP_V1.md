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

| Semantic area | Target descriptor | Current location | Gap kind | Next action |
|---|---|---|---|---|
| Bytecode control stream | instruction stream, entry PCs, slot counts | `Bytecode` | `implemented` | Document per-op semantic contract. |
| Procedure runtime metadata | procedure ids, params, return slot, entry | `ProcedureRuntimeMetadata` | `implemented-runtime-only` | Audit against full signature descriptor. |
| Slot declared type metadata | `SlotTypeDescriptor` | partial compiler/runtime metadata | `metadata-missing` | Add central `VbaTypeId` and slot descriptors. |
| Primitive carriers | scalar carrier/layout descriptors | runtime helpers, VM values | `metadata-missing` | Preserve declared primitive slots through package metadata. |
| Declared Variant | COM-compatible Variant carrier descriptor | `oxvba-runtime::Variant` | `implemented-runtime-only` | Bind declared `Variant` slots to package carrier metadata. |
| Decimal | Variant subtype/runtime payload | `Decimal96`, `VarType::Decimal` | `implemented-runtime-only` | Prevent declared Decimal storage except extension-gated rows. |
| Strings | `BStr`, fixed string descriptors | runtime `BStr`, compiler partials | `metadata-missing` | Add fixed/variable string descriptor and cleanup obligations. |
| Arrays | array type, shape, bounds, storage kind | runtime `SafeArray`, compiler partials | `metadata-missing` | Add shape/provenance descriptors and VM evidence. |
| UDTs | nominal type, fields, copy/init/cleanup | compiler/project metadata, runtime gaps | `metadata-missing` | Define package UDT descriptor and VM copy evidence. |
| Objects/classes/interfaces | object/class/interface descriptors | compiler/project/COM partials | `metadata-missing` | Unify `ObjectRef`, class, interface, COM imported descriptors. |
| Procedure signatures | full `ProcedureSignatureDescriptor` | compiler/project metadata | `metadata-missing` | Capture source/resolved mechanisms, optional/default, ParamArray, `Me`. |
| Expression classification | `ExpressionSemanticsDescriptor` | resolver/VM behavior | `metadata-missing` | Add package descriptor for value/variable/property/function/member shapes. |
| Let/Set coercion | `CoercionDescriptor` | runtime/compiler behavior | `metadata-missing` | Extract table and bind helpers to descriptor ids. |
| Operators | `OperatorSemanticsDescriptor` | bytecode/runtime helpers | `metadata-missing` | Build operator table and helper mapping. |
| Assignment/property | property accessor and value-param descriptors | compiler/VM behavior | `metadata-missing` | Audit `Get`/`Let`/`Set`, default member, property value ByVal semantics. |
| Call sites | `CallSiteDescriptor` | compiler/VM behavior | `metadata-missing` | Capture argument mapping, ByRef alias/temp, optional/default, ParamArray. |
| Error routing | error maps and resume targets | VM fields and bytecode | `implemented-runtime-only` | Add package error/resume map and evidence schema. |
| Cleanup/lifetime | cleanup obligation map | runtime helpers, VM paths | `metadata-missing` | Add slot lifecycle/cleanup descriptors. |
| COM projection | COM descriptor set | `oxvba-com`, compiler/project metadata | `interop-limitation` | Project from semantic descriptors, not raw wire types. |
| Native Declare | native ABI descriptor | `ExternalCallDescriptor` | `implemented-runtime-only` | Audit scalar/BSTR/Variant/SAFEARRAY/ByRef coverage. |
| Exported callable | inbound/outbound ABI descriptor | wrapper/export metadata | `metadata-missing` | Define package-level inbound projection and error policy. |
| Host capability policy | host requirement descriptors | HAL/host policy | `implemented-runtime-only` | Add digestable capability requirements to package. |
| Evidence schema | VM/JIT/package evidence | tracer seed tests | `test-shortcoming` | Record descriptor digests and package fact usage. |

## VM Rework Readiness Slices

The first VM rework should move from evidence and metadata toward behavior
only after the descriptor surfaces are visible and fixture-backed. These slices
are intentionally ordered so a broad storage rewrite is not the first step.

| Slice | Goal | Descriptor families | VM change type | Stop condition |
|---|---|---|---|---|
| VMR-01 | Package identity and procedure metadata | package digest, procedure id, bytecode digest, entry PC, slot counts | metadata/evidence only | VM run evidence records package and procedure identity without changing behavior. |
| VMR-02 | Slot descriptor surface | `SlotTypeDescriptor`, declared type ids, roles, initial states, carrier hints | metadata loading plus snapshots | VM can load and expose descriptors for parameters, locals, return slots, and temporaries while still executing existing slots. |
| VMR-03 | Signature descriptor surface | `ProcedureSignatureDescriptor`, `ParameterDescriptor`, return descriptor | metadata/evidence first | VM evidence can compare current call behavior against signature metadata. |
| VMR-04 | Expression and call descriptor seeds | `CallSiteDescriptor`, `ArgumentBindingDescriptor`, expression category descriptors | fixture-backed behavior audit | Fixtures classify call/argument gaps before behavior changes. |
| VMR-05 | Array, UDT, object descriptor seeds | `ArrayShapeDescriptor`, `UdtTypeDescriptor`, object/class/interface descriptors | metadata/evidence first | VM evidence captures shapes, field metadata, and object identity where current VM can execute. |
| VMR-06 | Behavior-affecting metadata consumption | selected call, array, coercion, and cleanup descriptors | targeted VM behavior changes | Each behavior change has a VM fixture and a completion-map gap classification. |

## Outstanding Ambiguities Before VM Rework

Resolve or explicitly classify these before behavior-affecting VM changes:

- Descriptor id ownership: decide whether ids are allocated by the compiler,
  package builder, bundle serializer, or a shared semantic registry.
- Carrier hint authority: decide whether first-pass carrier hints are
  authoritative execution inputs or observational evidence fields.
- Temporary slot modeling: define how compiler-generated temporaries receive
  stable names, roles, initial states, and cleanup obligations.
- In-memory package identity: define where package and descriptor digests live
  when execution does not flow through a persisted `OxBundle`.
- Partial resolver output: state which call-site and object/member descriptors
  can be emitted before the resolver is fully complete, and how unknowns are
  represented.
- Compatibility decisions: distinguish intentional VBA-compatible behavior from
  current implementation behavior before changing the VM to consume metadata.

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
