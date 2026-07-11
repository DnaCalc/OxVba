# VBA Type System v1

Status: current VBA semantic reference; implementation and evidence tracked in canonical matrices
Date: 2026-05-26
Authority review: 2026-07-11
Scope owner: VBA declared types and value-state semantics
System clauses: `AUTH-SPEC-001`, `COMP-BIND-001`, `RUNTIME-VALUE-001`, `IR-OXIR-001`
Current architecture: [`OXVBA_SYSTEM_CONTRACT_V1.md`](OXVBA_SYSTEM_CONTRACT_V1.md), [`OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md`](OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md)

## Purpose

Define the target VBA type model for full compatibility. This document refines
type-system meaning: source types, declared types, value states, callable
signatures, and the semantic facts required at runtime and external boundaries.

Authority follows `CHARTER.md`, `OPERATIONS.md`, and
[`OXVBA_SYSTEM_CONTRACT_V1.md`](OXVBA_SYSTEM_CONTRACT_V1.md). Public
specifications and reproducible black-box Excel/VBA observations decide
behavior. Current OxVba code and historical fixtures are regression evidence,
not semantic authority. A disagreement remains an explicit canonical matrix or
oracle row until adjudicated; it is never resolved in favor of current code by
default.

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
- Compiler semantic-fact contract:
  [`OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md`](OXVBA_COMPILER_AND_SEMANTIC_ANALYSIS_CONTRACT_V2.md)
- Verified OxIR/OxImage representation:
  [`OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md`](OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md)
- Runtime carrier layout:
  [`OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md`](OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md)
- Windows boundary projection:
  [`OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md`](OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md)

Rust-like structures in this document are non-normative illustrations of the
semantic facts that must remain distinguishable. They do not prescribe public
DTO names, crate ownership, artifact layout, VM slots, or JIT representation.
Those decisions belong to the active subsystem contracts.

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
   - The semantic carrier family: `Variant`, `BStr`, `ObjectRef`,
     `SafeArray`, descriptor-backed UDT fields, and selected control-plane
     tokens such as `BindingHandle`.
4. **External ABI/wire projection**
   - COM `VARIANT`/`BSTR`/`SAFEARRAY`/`IDispatch`, native `Declare` ABI shapes,
     and exported callable shapes. These are projections from declared semantic
     types and runtime carriers, not replacements for them.

The compiler-owned semantic facts and verified executable artifact preserve the
declared semantic type and enough descriptor information for consumers to
select a carrier and any external projection without rediscovering meaning.

Expression and call behavior is a companion semantic layer, not an optional
JIT detail. Declared type descriptors must be paired with the coercion,
operator, assignment, property, and call-site descriptors defined in
[`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md).
For example, a `ByRef Long` parameter is not described fully by the declared
type `Long`; its parameter mechanism, aliasing/writeback behavior, optional
state, and call-site compatibility checks are part of the same semantic fact
set preserved through the verified artifact.

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
- any compiler representation that can make `Decimal` look like ordinary
  declared storage remains a `CORE-READINESS/CORE-TYPED-BINDING` gap until it
  is rejected or explicitly extension-gated and evidenced.

### String Types

String descriptors must distinguish:

- variable-length `String`, represented internally by `BStr`;
- fixed-length `String * N`, including fixed strings in UDT fields;
- `vbNullString` as a value expression/projection case, not a distinct declared
  type.

Semantic and verified-artifact facts must record fixed length where present, default
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

Array bounds are shape metadata, not the element declared type. Compiler-owned
semantic facts and the verified artifact
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

Illustrative semantic fact shape (non-normative representation):

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

Internal storage is a backend decision, but it must preserve nominal UDT
semantics and ownership. JIT/native lowering must not treat a UDT as a platform
struct unless a separately verified ABI descriptor proves the layout.

### Enums

Enums are nominal declared types with `Long`-compatible values. Descriptors must
record:

- `EnumTypeId`;
- project/module/name identity and visibility;
- ordered member names and constant values;
- underlying carrier as `Long`;
- assignment/coercion behavior where VBA treats enum values as numeric.

Enum nominal identity, member order/value, declared-slot typing, and
enum-specific coercion diagnostics are required semantic facts. Their
implementation and evidence state is owned by
`CORE-READINESS/CORE-TYPED-BINDING` and the corresponding verified-artifact
rows; historical name-binding tokens do not establish current completion.

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

Procedure semantic facts must capture the full source signature and resolved
calling shape. This is required for VM3/JIT execution, COM projection, native
`Declare`, and exported callables.

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
- source span and resolved executable-procedure identity.

Illustrative semantic fact shape (non-normative representation):

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
    pub executable_procedure: Option<ResolvedProcedureId>,
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

COM and native projections consume verified semantic descriptors. Generated
code and runtime helpers must not discover these shapes through ambient symbols
or side channels; concrete descriptor representation belongs to the artifact
and Windows subsystem contracts.

## Spec Cross-Reference Review

This draft has been reviewed against the local Foundation MS-VBAL/MS-OAUT
extractions listed above. The current alignment target is:

| Area | Foundation/MS spec anchor | Type-system requirement |
|---|---|---|
| Scalar/value type list | MS-VBAL table 2/table 3, including `SEG-000585..000653` | Keep scalar declared types explicit; model `Decimal` as a runtime Variant subtype, not ordinary declared storage. |
| Decimal nuance | MS-VBAL `SEG-000653`; Microsoft Decimal language reference | `VarType::Decimal`/`Decimal96` is valid as Variant payload and COM/SAFEARRAY projection; declared `Dim x As Decimal` remains rejected or extension-gated. |
| Array declaration shape | MS-VBAL `CONF-...-0057..0059`, `SEG-001689..001691` | Distinguish scalar, resizable array, and fixed-size array declared types with element type and shape. |
| Array bounds constants | MS-VBAL `CONF-...-0062`, `SEG-001723` | Bounds expressions are compile-time values Let-coercible to `Long`; semantic facts preserve lower/upper/provenance. |
| Fixed string length | MS-VBAL `CONF-...-0066`, `SEG-001744..001745` | Fixed-length String descriptors carry length provenance and assignment rules. |
| Procedure parameter grammar | MS-VBAL `CONF-...-0107..0129`, `SEG-002047..002101` | Signature descriptors include parameter order, role, ByRef/ByVal/defaulting, array designators, Optional, ParamArray, and implicit `Me`. |
| Property signatures | MS-VBAL `CONF-...-0134..0139`, `SEG-002107..002121` | Property descriptors preserve Get/Let/Set grouping, value-param declared type, and runtime ByVal value-param semantics. |
| Event signatures | MS-VBAL `CONF-...-0178`, `SEG-001918`, `SEG-002488` | Event declaration parameters are signature facts and handler compatibility inputs, not ordinary locals. |
| Object/class conformance | MS-VBAL `CONF-...-0008..0009`; Set-coercion sections | Object descriptors include class/interface conformance and `Nothing` state. |
| COM wire types | MS-OAUT `CONF-...-0010..0016`, `0023..0028`, `0042`, `0050..0077`, `0173..0188`, `0202`, `0224`, `0227`, `0278..0287` | COM descriptors project to VARIANT/BSTR/SAFEARRAY/IDispatch/IUnknown without replacing the core OxVba type model. |

Canonical closure routes:

- stable declared-type identities, complete callable facts, and compiler
  diagnostics are owned by `CORE-READINESS/CORE-TYPED-BINDING`;
- verified nominal types, arrays, records, callable signatures, and consumer
  admission are owned by the applicable `OXIR-BACKENDS` and
  `OXIMAGE-CONTRACT` rows;
- Let/Set coercion and operator truth tables are specified in
  [`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md).
  Historical seed rows remain non-authoritative coverage inputs at
  [`../validation/VBA_COERCION_SEED_TABLE_V1.csv`](../validation/VBA_COERCION_SEED_TABLE_V1.csv);
  operator inputs are at
  [`../validation/VBA_OPERATOR_SEED_TABLE_V1.csv`](../validation/VBA_OPERATOR_SEED_TABLE_V1.csv);
  lifecycle inputs are at
  [`../validation/VBA_LIFECYCLE_CLEANUP_SEED_TABLE_V1.csv`](../validation/VBA_LIFECYCLE_CLEANUP_SEED_TABLE_V1.csv);
  and object/member inputs are at
  [`../validation/VBA_OBJECT_MEMBER_BINDING_SEED_TABLE_V1.csv`](../validation/VBA_OBJECT_MEMBER_BINDING_SEED_TABLE_V1.csv);
  they gain current completion credit only through replay on canonical rows;
- `Array(...)` lower-bound behavior and any other unresolved observable receive
  an authoritative public-spec reason or a current Excel/VBA oracle row before
  VM3/JIT specialization is accepted.

## Semantic Slot Facts

Consumers require the following per-slot facts. The shape is illustrative and
non-normative:

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
}
```

This is not a storage mandate. Backends may choose physical slots only after
preserving the verified declared type, carrier, ownership, and call facts.
`Unknown` or poison type facts are permitted only in editor analysis and cannot
enter verified Core IR/OxIR or code generation. Internal binding handles are
control-plane data, not VBA slot carriers. `CallerProvided` is the initial state
for parameter slots before call-binding facts define the exact source,
aliasing, and writeback behavior.

## Historical Implementation Snapshot Disposition

The removed compiler/Bundle/VM inventory that originally occupied this section
was a 2026-05-26 implementation snapshot. It is intentionally not retained as
active guidance: its crate paths, package versions, descriptor tokens, VMR
milestones, and gap claims are historical provenance in Git and cannot establish
current architecture, expected VBA behavior, or completion. Current realization
is recorded in `docs/ARCHITECTURE.md`; current delivery and evidence state comes
only from the accepted worksets and canonical matrices.

Any still-useful fixture or observation from that snapshot must be replayed on
the current compiler -> Core IR -> verified OxIR/OxImage -> VM3/JIT route and
adjudicated against public specifications or reproducible Excel/VBA behavior
before it can advance a compatibility row.

## Required Type-System Direction

1. Preserve declared types and callable signatures in compiler AnalysisResult, Core IR and OxIR.
2. Give OxIR/OxImage stable nominal type/record/interface identities and verified descriptors.
3. Keep exact runtime carriers distinct from declared VBA type meaning.
4. Make VM3 and JIT consume the same verified type/storage/call descriptors.
5. Use backend lowering plans only for physical representation and calling convention.
6. Gate non-VBA extension types behind explicit diagnostics and separate capability rows.
