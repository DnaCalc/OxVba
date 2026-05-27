# VBA Type System v1

Status: `working-draft` authoritative reference for new package, VM, JIT, and
interop type work
Date: 2026-05-26
Scope owner: OxVBA compiler/VM/runtime/COM/native-readiness

## Purpose

Define the target OxVba type model for full VBA compatibility and future JIT
work. This document is the development authority for type-system shape: new
compiler metadata, VM package metadata, `ProcLoweringIr`, COM/native
descriptors, and differential harness evidence should align to this model.

Current code remains executable truth where implementation and this draft are
not yet aligned. Such differences are gaps to close or explicitly classify, not
permission to invent a parallel JIT-only type model.

## Source References

Primary public references used by this draft:

- Microsoft VBA data type summary:
  <https://learn.microsoft.com/office/vba/language/reference/user-interface-help/data-type-summary>
- `Dim` statement:
  <https://learn.microsoft.com/office/vba/Language/Reference/User-Interface-Help/dim-statement>
- `Decimal` data type:
  <https://learn.microsoft.com/office/vba/language/reference/user-interface-help/decimal-data-type>
- Deftype statements:
  <https://learn.microsoft.com/office/vba/language/concepts/getting-started/deftype-statements>
- `Object` data type:
  <https://learn.microsoft.com/office/vba/language/reference/user-interface-help/object-data-type>
- `Set` statement and `Nothing`:
  <https://learn.microsoft.com/office/vba/language/reference/user-interface-help/set-statement>
- `Implements` statement:
  <https://learn.microsoft.com/office/vba/language/reference/user-interface-help/implements-statement>
- `WithEvents` keyword:
  <https://learn.microsoft.com/office/vba/language/reference/user-interface-help/withevents-keyword>

Canonical spec extraction references:

- Foundation source map:
  [`../FOUNDATION_SPEC_REFERENCE.md`](../FOUNDATION_SPEC_REFERENCE.md)
- MS-VBAL extracted conformance set:
  `../../../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/conformance_items.jsonl`
- MS-VBAL extracted spec items:
  `../../../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/spec_items.jsonl`
- MS-VBAL extracted segments:
  `../../../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/docs/discovered-ms-vbal-250520-f945507e/segments.jsonl`
- MS-OAUT extracted conformance set:
  `../../../Foundation/reference/runs/20260301-ms-oaut-pass02/outputs/conformance_items.jsonl`

Project-specific companion references:

- Expression, coercion, and call semantics:
  [`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md)
- Executable package target:
  [`EXECUTABLE_SEMANTIC_PACKAGE_V1.md`](EXECUTABLE_SEMANTIC_PACKAGE_V1.md)
- Native-ready value substrate:
  [`NATIVE_READY_VALUE_SUBSTRATE_V1.md`](NATIVE_READY_VALUE_SUBSTRATE_V1.md)
- COM early binding scope:
  [`COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md`](COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md)
- COM client/server scope:
  [`COM_CLIENT_SERVER_SCOPE_V1.md`](COM_CLIENT_SERVER_SCOPE_V1.md)

## Layer Model

OxVba must not collapse all type information into one enum. Four layers are
required:

1. **Source type syntax**
   - The spelling and local context from source: `As Long`, `As String * 10`,
     type characters, `DefLng A-Z`, `As New Foo.Bar`, `WithEvents`,
     `ParamArray`, `Optional`, qualification, and imported-library names.
2. **Declared semantic type**
   - The resolved VBA type after name lookup, project/reference binding,
     `DefType` rules, type characters, module kind rules, and object/interface
     resolution.
3. **Runtime carrier**
   - The current execution carrier: retained `Variant`, `BStr`, `ObjectRef`,
     `SafeArray`, descriptor-backed UDT fields, and selected control-plane
     tokens such as `BindingHandle`.
4. **External ABI/wire projection**
   - COM `VARIANT`/`BSTR`/`SAFEARRAY`/`IDispatch`, native `Declare` ABI shapes,
     and exported callable shapes. These are projections from declared semantic
     types and runtime carriers, not replacements for them.

The executable semantic package carries the declared semantic type and enough
descriptor metadata to choose the runtime carrier and external projection.

Expression and call behavior is a companion semantic layer, not an optional
JIT detail. Declared type descriptors must be paired with the coercion,
operator, assignment, property, and call-site descriptors defined in
[`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md).
For example, a `ByRef Long` parameter is not described fully by the declared
type `Long`; its parameter mechanism, aliasing/writeback behavior, optional
state, and call-site compatibility checks are part of the same executable
semantic package.

## Declared Type Categories

### Scalar Declared Types

Supported VBA scalar declared types:

- `Boolean`
- `Byte`
- `Integer`
- `Long`
- `LongLong`
- `LongPtr`
- `Single`
- `Double`
- `Currency`
- `Date`
- `String`
- `Variant`

`LongLong` and `LongPtr` are VBA7/platform-sensitive. `LongPtr` is a source
semantic type whose runtime carrier depends on target pointer width.

Default initialization:

- numeric scalars: zero value of the declared type;
- `Boolean`: `False`;
- variable-length `String`: empty string;
- object/class/interface variables: `Nothing`;
- `Variant`: `Empty`;
- dynamic arrays: unallocated array descriptor state;
- fixed arrays and UDT fields: recursively initialized element/field defaults.

### Decimal

`Decimal` is not an ordinary declared variable type in VBA. It is a Variant
subtype produced by Variant-valued expressions such as `CDec(...)` and by COM
or runtime boundaries that yield `VT_DECIMAL`.

OxVba rules:

- runtime `Variant` must support `VarType::Decimal`;
- the payload is `Decimal96`: 96-bit integer magnitude plus scale/sign metadata;
- `Decimal` may appear as a runtime value tag, SAFEARRAY element carrier, COM
  projection, and diagnostic/snapshot tag;
- `Dim x As Decimal`, `DefDec`, and ordinary declared Decimal slots are not
  spec-compatible declared-type lanes and must be rejected or placed behind an
  explicit non-VBA extension gate;
- compiler internals that currently contain a `Decimal` bound type must be
  audited so they cannot make Decimal look like a normal declared storage type
  for package/JIT purposes.

### String Types

String descriptors must distinguish:

- variable-length `String`, represented internally by `BStr`;
- fixed-length `String * N`, including fixed strings in UDT fields;
- `vbNullString` as a value expression/projection case, not a distinct declared
  type.

String package metadata must record fixed length where present, default
initialization, assignment truncation/padding behavior, and cleanup ownership.

### Arrays

Array descriptors must record:

- dynamic versus fixed/static array;
- rank;
- declared bounds per dimension where known;
- `Option Base` influence for omitted lower bounds;
- element declared type;
- element runtime carrier;
- resize/`ReDim`/`Preserve` legality;
- array-as-parameter shape and ByRef writeback requirements;
- fixed array field shape inside UDTs.

Arrays are declared type constructors. `SafeArray` is the runtime/interop
carrier used for array values and COM projection.

### Array Bounds And Shape

Array bounds are shape metadata, not the element declared type. The package
must preserve both:

- the element declared type and carrier;
- the rank, lower/upper bound, and provenance of each dimension.

Static/fixed arrays and fixed array fields in UDTs carry their shape in the
declaration/storage descriptor. Dynamic arrays carry their element type in the
declared type, while their current shape is runtime allocation state produced
by `ReDim`, array literal/runtime helpers, ParamArray binding, or COM SAFEARRAY
projection.

Bounds rules:

- explicit `lower To upper` clauses win over any module default;
- omitted lower bounds use the applicable `Option Base` default at compile
  time, and the descriptor must record that provenance;
- `ReDim` changes the runtime allocation shape of a resizable array, not its
  element declared type;
- `ReDim Preserve` cannot change rank or element type and can resize only the
  last dimension under VBA-compatible rules;
- `LBound` and `UBound` read runtime shape, and dimension arguments are
  one-based;
- `ParamArray` binding creates a Variant array with lower bound 0, including
  the empty lower-0/upper--1 shape when there are no extra arguments;
- `Array(...)` function lower-bound behavior must be represented by
  `ArrayFunction` provenance and closed against MS-VBAL plus Office oracle
  evidence before JIT specialization relies on it;
- COM SAFEARRAY projection must preserve the lower bound and element type
  metadata per dimension exactly at the boundary.

Target package shape:

```rust
pub enum ArrayStorageKind {
    Dynamic,
    Fixed { shape: ArrayShapeDescriptor },
    StaticLocal { shape: ArrayShapeDescriptor },
    UdtFieldFixed { shape: ArrayShapeDescriptor },
    ParamArray,
    ComSafeArrayProjection,
}

pub struct ArrayShapeDescriptor {
    pub rank: usize,
    pub dimensions: Vec<ArrayDimensionBounds>,
    pub provenance: ArrayShapeProvenance,
}

pub struct ArrayDimensionBounds {
    pub lower: i32,
    pub upper: i32,
}

pub enum ArrayShapeProvenance {
    ExplicitToClause,
    OptionBaseDefault,
    RuntimeReDim,
    ReDimPreserve,
    ArrayFunction,
    ParamArrayBinding,
    ComSafeArray,
}
```

### UDTs

UDTs are nominal declared types, not structural aliases. Descriptors must
record:

- `UdtTypeId`;
- project/module/name identity and visibility;
- ordered fields;
- each field's declared type id;
- fixed-string lengths;
- fixed-array field bounds and element type;
- nested UDT field references;
- initialization, copy, assignment, and cleanup rules;
- native ABI materialization policy, explicitly separate from internal
  descriptor-backed semantics.

Internal storage may remain flattened/descriptor-backed over retained slots for
now. JIT/native lowering must not treat a UDT as a platform struct unless a
separate ABI descriptor proves the layout.

### Enums

Enums are nominal declared types with `Long`-compatible values. Descriptors must
record:

- `EnumTypeId`;
- project/module/name identity and visibility;
- ordered member names and constant values;
- underlying carrier as `Long`;
- assignment/coercion behavior where VBA treats enum values as numeric.

### Object, Class, Interface, And Imported COM Types

Object-like declared types are central, not edge cases.

Descriptor forms:

- `Object`: late-bound object reference.
- `Class { class_id }`: VBA class module object type.
- `Interface { interface_id }`: interface implemented by a class or imported
  from type information.
- `ComClass { typelib_id, coclass_id }`: imported COM coclass.
- `ComInterface { typelib_id, interface_id, dispatch_kind }`: imported COM
  interface, dispatch interface, or dual interface.
- `WithEventsObject { object_type, event_source_id }`: object variable that
  participates in event binding.
- `AsNewObject { object_type, activation_policy }`: object variable with
  automatic instantiation semantics.

Required object metadata:

- object identity carrier: `ObjectRef`;
- default interface and default member metadata;
- `IDispatch`/vtable strategy for imported COM types;
- `Implements` interface mapping from class member implementations to interface
  members;
- `WithEvents` variable identity and event-handler prefix binding;
- `RaiseEvent` and event dispatch signatures;
- `Set` assignment compatibility, `Nothing`, and object lifetime rules;
- class initialization/termination hooks where in scope;
- default instance behavior where in scope.

`Nothing` is a value state for object variables, not a declared type.

## Value States That Are Not Declared Types

These must be represented explicitly as runtime value states, expression states,
or call argument states, not ordinary declared types:

- `Empty`: default value of a `Variant`.
- `Null`: Variant/database null value state.
- `Error`/`CVErr`: Variant error value state.
- `Nothing`: object reference value state.
- missing optional argument: call argument state for `Optional` parameters.
- omitted/default parameter value: call binding state, with optional default
  value metadata.
- `vbNullString`: string value/projection state, not a declared type.

JIT and VM snapshots may observe these states through retained `Variant`,
`ObjectRef`, or call-frame metadata, but they must not appear as declared slot
types.

## Procedure Type Descriptors

Procedure descriptors must capture the full source signature and the resolved
semantic calling shape. This is required for ordinary VM execution, COM
projection, native Declare, exported callables, and future JIT call lowering.

Procedure descriptors must record:

- procedure kind: `Sub`, `Function`, `Property Get`, `Property Let`,
  `Property Set`, event declaration, event handler, external Declare, exported
  callable;
- module/class/interface ownership;
- visibility and module kind;
- procedure name and property/event pairing identity;
- parameter list in source order;
- parameter names and case-insensitive name identity;
- parameter role: positional, optional, ParamArray, property value parameter,
  implicit current object, event signature parameter;
- source parameter mechanism: explicit `ByRef`, explicit `ByVal`, or omitted;
- resolved runtime mechanism, including the default `ByRef` rule and the
  special property-LHS value-parameter ByVal semantics;
- `Optional` and default-value expression/data value;
- missing optional argument state where distinct from default assignment;
- `ParamArray` element type, Variant-array shape, and binding rules;
- declared parameter type id;
- parameter array designator and resizable-array parameter type where present;
- return type id where applicable;
- property accessor pairing rules and property value type;
- event signature compatibility and handler binding;
- implicit method current-object (`Me`) descriptor;
- external Declare ABI surface where applicable;
- source span and bytecode entry metadata.

Target signature model:

```rust
pub struct ProcedureSignatureDescriptor {
    pub procedure_id: ProcedureId,
    pub name: NameId,
    pub owner: ProcedureOwnerId,
    pub kind: ProcedureKind,
    pub visibility: ProcedureVisibility,
    pub parameters: Vec<ParameterDescriptor>,
    pub return_type: Option<VbaTypeId>,
    pub property_group: Option<PropertyGroupId>,
    pub event_signature: Option<EventSignatureDescriptor>,
    pub implicit_current_object: Option<ImplicitCurrentObjectDescriptor>,
    pub external_abi: Option<ExternalAbiDescriptorId>,
    pub source_span: Option<SourceSpanId>,
    pub bytecode_entry: Option<BytecodeEntryId>,
}

pub struct ParameterDescriptor {
    pub index: usize,
    pub name: NameId,
    pub role: ParameterRole,
    pub source_mechanism: SourceParameterMechanism,
    pub resolved_mechanism: ResolvedParameterMechanism,
    pub declared_type: VbaTypeId,
    pub array_parameter: Option<ArrayParameterDescriptor>,
    pub optional: OptionalParameterDescriptor,
    pub param_array: Option<ParamArrayDescriptor>,
    pub slot: Option<usize>,
}

pub enum ParameterRole {
    Positional,
    Optional,
    ParamArray,
    PropertyValue,
    ImplicitCurrentObject,
    EventSignatureOnly,
}

pub enum SourceParameterMechanism {
    Omitted,
    ExplicitByRef,
    ExplicitByVal,
    ImplementationInjected,
}

pub enum ResolvedParameterMechanism {
    ByRef,
    ByVal,
    PropertyValueByVal,
    EventSignatureOnly,
}

pub enum OptionalParameterDescriptor {
    Required,
    Optional {
        default_value: OptionalDefaultValue,
        missing_state: OptionalMissingStatePolicy,
    },
}

pub enum OptionalDefaultValue {
    Explicit(ConstantValueId),
    DeclaredTypeDefault,
    VariantMissingError448,
    ImplementationDefined,
}

pub enum OptionalMissingStatePolicy {
    AssignDefaultLocal,
    PreserveMissingArgumentState,
}

pub struct ParamArrayDescriptor {
    pub element_type: VbaTypeId,
    pub array_lower_bound: i32,
    pub empty_upper_bound: i32,
}

pub struct ImplicitCurrentObjectDescriptor {
    pub declared_type: VbaTypeId,
    pub mechanism: ResolvedParameterMechanism,
    pub accessible_name: NameId,
    pub assignable: bool,
}
```

`ByRef` compatibility is exact declared type compatibility for non-object,
non-Variant parameters unless a documented VBA rule permits a different
binding. `ByVal` may coerce according to assignment and call-context rules.
Call-site aliasing, temporary-local creation, optional/default assignment,
ParamArray construction, and boundary writeback are defined in
[`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md).

## Interop Type Projection

Interop descriptors project from semantic types:

- COM projection: `VARIANT`, `BSTR`, `SAFEARRAY`, `IDispatch`, `IUnknown`,
  vtable interface pointer, `EXCEPINFO`, HRESULT/ArgErr behavior.
- Native `Declare` projection: platform calling convention, pointer width,
  string/array/variant marshaling, ByRef writeback, return value projection.
- Exported callable projection: inbound ABI shape, slot population, ByRef
  writeback, cleanup, and error return policy.

COM and native descriptors are package metadata. Generated code and JIT helpers
must not discover these shapes through ambient symbols or side channels.

## Spec Cross-Reference Review

This draft has been reviewed against the local Foundation MS-VBAL/MS-OAUT
extractions listed above. The current alignment target is:

| Area | Foundation/MS spec anchor | Type-system requirement |
|---|---|---|
| Scalar/value type list | MS-VBAL table 2/table 3, including `SEG-000585..000653` | Keep scalar declared types explicit; model `Decimal` as a runtime Variant subtype, not ordinary declared storage. |
| Decimal nuance | MS-VBAL `SEG-000653`; Microsoft Decimal language reference | `VarType::Decimal`/`Decimal96` is valid as Variant payload and COM/SAFEARRAY projection; declared `Dim x As Decimal` remains rejected or extension-gated. |
| Array declaration shape | MS-VBAL `CONF-...-0057..0059`, `SEG-001689..001691` | Distinguish scalar, resizable array, and fixed-size array declared types with element type and shape. |
| Array bounds constants | MS-VBAL `CONF-...-0062`, `SEG-001723` | Bounds expressions are compile-time values Let-coercible to `Long`; package records lower/upper/provenance. |
| Fixed string length | MS-VBAL `CONF-...-0066`, `SEG-001744..001745` | Fixed-length String descriptors carry length provenance and assignment rules. |
| Procedure parameter grammar | MS-VBAL `CONF-...-0107..0129`, `SEG-002047..002101` | Signature descriptors include parameter order, role, ByRef/ByVal/defaulting, array designators, Optional, ParamArray, and implicit `Me`. |
| Property signatures | MS-VBAL `CONF-...-0134..0139`, `SEG-002107..002121` | Property descriptors preserve Get/Let/Set grouping, value-param declared type, and runtime ByVal value-param semantics. |
| Event signatures | MS-VBAL `CONF-...-0178`, `SEG-001918`, `SEG-002488` | Event declaration parameters are signature facts and handler compatibility inputs, not ordinary locals. |
| Object/class conformance | MS-VBAL `CONF-...-0008..0009`; Set-coercion sections | Object descriptors include class/interface conformance and `Nothing` state. |
| COM wire types | MS-OAUT `CONF-...-0010..0016`, `0023..0028`, `0042`, `0050..0077`, `0173..0188`, `0202`, `0224`, `0227`, `0278..0287` | COM descriptors project to VARIANT/BSTR/SAFEARRAY/IDispatch/IUnknown without replacing the core OxVba type model. |

Review gaps:

- the central `VbaTypeId` registry does not exist yet;
- the compiler/VM metadata still needs auditing against the full
  `ProcedureSignatureDescriptor` shape;
- Let/Set coercion and operator truth tables are tracked in
  [`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md).
  The first coercion seed rows are checked in at
  [`../validation/VBA_COERCION_SEED_TABLE_V1.csv`](../validation/VBA_COERCION_SEED_TABLE_V1.csv);
  the first operator seed rows are checked in at
  [`../validation/VBA_OPERATOR_SEED_TABLE_V1.csv`](../validation/VBA_OPERATOR_SEED_TABLE_V1.csv);
  the first lifecycle/cleanup seed rows are checked in at
  [`../validation/VBA_LIFECYCLE_CLEANUP_SEED_TABLE_V1.csv`](../validation/VBA_LIFECYCLE_CLEANUP_SEED_TABLE_V1.csv);
  the first object/member binding seed rows are checked in at
  [`../validation/VBA_OBJECT_MEMBER_BINDING_SEED_TABLE_V1.csv`](../validation/VBA_OBJECT_MEMBER_BINDING_SEED_TABLE_V1.csv);
  full truth-table extraction and canonical descriptor ids remain open;
- `Array(...)` function lower-bound behavior needs explicit MS-VBAL plus Office
  oracle closure before specialization.

## Package Slot Descriptor Target

The executable semantic package should expose a per-slot descriptor shaped like:

```rust
pub struct SlotTypeDescriptor {
    pub slot: usize,
    pub name: Option<String>,
    pub role: SlotRole,
    pub declared_type: VbaTypeId,
    pub initial_state: SlotInitialState,
    pub carrier: RuntimeCarrierKind,
}

pub enum SlotRole {
    Parameter,
    Local,
    ReturnValue,
    Temporary,
    CompilerGenerated,
}

pub enum SlotInitialState {
    Unknown,
    CallerProvided,
    Empty,
    ScalarZero,
    False,
    EmptyString,
    Nothing,
    UnallocatedArray,
    UdtDefault,
    CompilerDefined,
}

pub enum RuntimeCarrierKind {
    Unknown,
    Variant,
    Boolean,
    I16,
    U8,
    I32,
    I64,
    PointerSizedInteger,
    F32,
    F64,
    Currency,
    Date,
    BStr,
    Decimal96VariantSubtype,
    ObjectRef,
    SafeArray,
    UdtFields { descriptor: UdtTypeId },
    BindingHandleInternal,
}
```

This is a target package model, not an immediate storage mandate. The VM may
continue executing retained `Variant` slots while exposing declared type and
carrier metadata for evidence and future lowering.

`Unknown` is a temporary package-strengthening state for metadata that current
compiler/runtime surfaces do not yet preserve. It is not a type-system answer
and must be treated as unsupported by JIT/native entry gates. `CallerProvided`
is the initial state for parameter slots before call-binding descriptors define
the exact argument source, aliasing, and writeback behavior.

## Current Code Anchors And Gaps

Current implementation anchors:

- `crates/oxvba-runtime/src/variant.rs`: runtime `VarType` tags and retained
  `Variant` storage.
- `crates/oxvba-runtime/src/decimal.rs`: `Decimal96`.
- `crates/oxvba-runtime/src/safe_array.rs`: SAFEARRAY element carriers,
  including Decimal element support.
- `crates/oxvba-compiler/src/resolve.rs`: current `BoundType`,
  `BoundArrayDescriptor`, `BoundParam`, external declaration shape, and
  project/module binding.
- `crates/oxvba-compiler/src/project.rs`: current reflection
  `VbaTypeDescriptor`, procedure/class/export metadata, and imported COM
  descriptors.
- `crates/oxvba-compiler/src/emit.rs`: current `ProcedureRuntimeMetadata` and
  `ProcedureRuntimeSlotMetadata`; `SlotTypeDescriptor` view with provisional
  `VbaTypeId`, `SlotInitialState`, and `RuntimeCarrierKind` enums populated
  for parameters, locals, return slots, compiler-generated fixed-array element
  slots, and temporary slots; first `ProcedureSignatureDescriptor` and
  `ParameterDescriptor` view for procedure kind, parsed parameter mode,
  source/resolved parameter mechanism, Optional/default/missing policy,
  ParamArray shape, return type, property group, property value ByVal
  semantics, and class hidden-receiver metadata; first `ArrayShapeDescriptor`
  rows for resolver-known arrays, including rank, declared bounds, storage
  kind, `Option Base`, element type, and element carrier; and first
  `UdtTypeDescriptor` rows for nominal UDT ids, instances, fields, nested UDT
  references, fixed strings, fixed array fields, aliases, and cleanup flags;
  and first `ObjectTypeDescriptor` rows for generic `Object` slots with
  `Nothing` initial state, `ObjectRef` carrier, activation/event/default
  member policy, support classification, and per-slot instances.
- `crates/oxvba-compiler/src/bundle.rs`: `OxBundle` format v10 serializes the
  populated slot, signature, seed call-site, array-shape, UDT, and object
  metadata and upgrades v1/v2/v3/v4/v5/v6/v7/v8/v9 bundles into the current
  descriptor shape.
- `crates/oxvba-vm/src/interpreter.rs`: `VmExecutionPackage` and package
  metadata loading; `VmExecutionPackage::slot_type_descriptors` exposes the
  current slot descriptor view and
  `VmExecutionPackage::procedure_signature_descriptors` exposes the first
  signature descriptor view; `VmExecutionPackage::call_site_descriptors`
  exposes seed call-site rows without changing slot or call execution; and
  `VmPackageIdentityEvidence` reports per-procedure slot descriptor digests,
  descriptor rows, signature/call observation rows for current seed `CallProc`
  lowering compared with signature metadata, and call-site descriptor evidence
  rows for the first VMR-04 fixtures, plus array-shape and UDT descriptor
  evidence for VMR-05 fixtures; object descriptor evidence for generic object
  slots; and runtime project evidence for VM-capable class/interface dynamic
  object routes and imported COM `WithEvents` routes when those route tables
  are supplied.
- `conformance/vm_package/identity_seed`: VM-runnable package fixtures assert
  value snapshots plus descriptor tokens for primitive scalar, `String`/`BStr`,
  declared `Variant`, the current flattened UDT field-alias/base-slot shape,
  first nominal UDT descriptors, and VMR-03 call observations for ByVal,
  ByRef, Optional default, ParamArray, property value, and return copyout
  behavior. VMR-04 fixture rows add call-site
  descriptor evidence for ByRef alias/writeback, ByRef expression temp,
  ByVal copy with the selected package-backed declared-`Double` call-entry
  coercion shape, Optional default, Optional `Variant` missing-policy metadata,
  and empty/non-empty `ParamArray`
  shape. The VMR-05 fixture rows assert fixed/static array descriptor bounds,
  dynamic `ReDim` runtime SAFEARRAY bounds, `Option Base` influence, element
  carrier facts, package-backed rank-1 fixed/static `LBound`/`UBound`
  descriptor consumption with a raw-bytecode base-slot limitation baseline, and
  UDT descriptor facts for nested UDTs, fixed strings, fixed array fields,
  aliases, cleanup ownership flags, and generic `Object` descriptor facts. A companion
  project fixture asserts source-project class route identity, implemented
  interface alias identity, and imported COM `WithEvents` route identity for
  current VM-capable route tables.

Known development gaps:

- no central `VbaTypeId`/descriptor registry exists yet;
- `ProcedureSignatureDescriptor` now carries the first call-relevant source
  and resolved parameter facts, missing optional state, ParamArray shape,
  implicit `Me`, and property value ByVal semantics, but signature descriptor
  ids, complete call-site coverage, and descriptor-driven VM call behavior
  remain incomplete;
- `CallSiteDescriptor` and `ArgumentBindingDescriptor` now represent the first
  top-level project call sites with ByRef alias/writeback, ByRef expression
  temporary/no-writeback, ByVal copy, Optional/default and Optional `Variant`
  missing-policy metadata, named/omitted, ParamArray, fixed-array
  materialization, default-member fallback policy, property value ByVal
  classification, and return copyout facts. Expression-call coverage outside
  current direct lowering, external Declare/COM call coverage, canonical
  descriptor ids, broader descriptor-driven VM behavior, broad ByVal call-entry
  coercion, and true Optional-missing runtime behavior remain incomplete. The
  first VMR-06 path consumes package metadata only for direct local `Long` to
  declared-`Double ByVal` call entry;
- `ProcedureRuntimeSlotMetadata`, `ArrayShapeDescriptor`, `UdtTypeDescriptor`,
  and `ObjectTypeDescriptor` now carry first-pass slot, local array shape, UDT
  shape facts, and generic object slot facts, but expression temporary declared types,
  multi-rank/error/lifecycle array evidence, aggregate UDT field offsets/layout,
  descriptor-driven UDT copy/drop behavior, richer class/interface/imported-COM
  descriptors, `As New` activation/default-instance policy, general
  fixed-string behavior, cleanup maps, and full carrier layout facts remain
  incomplete or explicitly `Unknown`; the first lifecycle/cleanup seed table
  names current cleanup obligations for primitive, Variant/Decimal, BStr,
  SafeArray, ObjectRef, UDT fields, ByRef temps, COM/native boundary temps, and
  deopt state, but those rows are not package-owned descriptors yet;
- current `BoundType::Decimal` must be audited so Decimal is retained as a
  Variant subtype/value carrier rather than accepted as ordinary declared
  storage;
- project reflection `VbaType` is useful but too coarse for package/JIT use;
- enum descriptors, UDT offset/layout/lifecycle descriptors, richer
  object/class/interface descriptors, and COM imported type descriptors are not
  yet unified behind one package type registry; the first object/member binding
  seed table now names `Set`/`Nothing`, property accessors, default members,
  class/interface routes, COM dispatch, early-bound COM, events, and
  `WithEvents` as binding rows, but those rows are not a package-owned member
  registry yet.

## Implementation Direction

1. Add a compiler/package type registry with stable `VbaTypeId` values.
2. Populate slot descriptors for parameters, locals, return values, temporaries,
   and compiler-generated slots.
3. Serialize descriptors through `OxBundle` or its executable semantic package
   successor.
4. Teach VM package execution and snapshot evidence to expose declared slot
   metadata without changing current retained-`Variant` execution.
5. Use the same descriptors as the input to `ProcLoweringIr`.
6. Gate any non-VBA extension types, including declared Decimal storage, behind
   explicit diagnostics and support-matrix rows.
