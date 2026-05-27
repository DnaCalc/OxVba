# VBA Expression And Call Semantics v1

Status: `working-draft`
Date: 2026-05-26
Scope owner: OxVBA compiler/VM/runtime/JIT/native-readiness

## Purpose

Define the expression, coercion, assignment, and procedure-invocation semantics
that sit beside the declared type model in
[`VBA_TYPE_SYSTEM_V1.md`](VBA_TYPE_SYSTEM_V1.md).

The type-system reference answers "what type is declared here?" This document
answers "what happens when values, expressions, properties, and calls are
evaluated?" Both documents are required inputs to the executable semantic
package, VM evidence, `ProcLoweringIr`, COM/native descriptors, and future JIT
lowering.

This is not a JIT-only contract. If a JIT needs a coercion, operator, call-site,
default-member, or ByRef fact, that fact belongs in the executable semantic
package first and must be observable by the VM or VM evidence.

## Source References

Primary canonical sources are managed in the sibling Foundation repository:

- Source map:
  [`../FOUNDATION_SPEC_REFERENCE.md`](../FOUNDATION_SPEC_REFERENCE.md)
- MS-VBAL extracted conformance set:
  `../../../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/conformance_items.jsonl`
- MS-VBAL extracted spec items:
  `../../../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/spec_items.jsonl`
- MS-VBAL extracted segments:
  `../../../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/docs/discovered-ms-vbal-250520-f945507e/segments.jsonl`
- MS-OAUT extracted conformance set:
  `../../../Foundation/reference/runs/20260301-ms-oaut-pass02/outputs/conformance_items.jsonl`

Local review anchors used by this draft:

- MS-VBAL 5.5.1, Let-coercion:
  `SPEC-discovered-ms-vbal-250520-f945507e-00185..00201`.
- MS-VBAL 5.5.2, Set-coercion:
  `SPEC-discovered-ms-vbal-250520-f945507e-00202..00206`.
- MS-VBAL 5.6.9, operator expressions:
  `SPEC-discovered-ms-vbal-250520-f945507e-00219..00246`.
- Parameter grammar and signature facts:
  `CONF-discovered-ms-vbal-250520-f945507e-0107..0129`.
- Property declaration and property value-parameter facts:
  `CONF-discovered-ms-vbal-250520-f945507e-0134..0139`,
  `discovered-ms-vbal-250520-f945507e-SEG-002107..002121`.
- Procedure invocation argument mapping:
  `discovered-ms-vbal-250520-f945507e-SEG-002162..002189`.
- Event signature compatibility:
  `CONF-discovered-ms-vbal-250520-f945507e-0178`.
- COM Automation projection:
  `CONF-discovered-ms-oaut-240423-b76f9b41-0010..0016`,
  `0023..0028`, `0042`, `0050..0077`, `0173..0188`,
  `0202`, `0224`, `0227`, `0278..0287`.

## Relationship To The Type System

`VBA_TYPE_SYSTEM_V1.md` owns:

- declared type categories;
- value states that are not declared types;
- runtime carrier families;
- procedure and parameter descriptors;
- array, UDT, object/class/interface, COM, and interop type descriptors.

This document owns:

- expression classification and value production;
- Let-coercion and Set-coercion descriptors;
- operator result typing and error behavior;
- Let/Set assignment, property access, and default-member effects;
- procedure call argument mapping, optional/default handling, ParamArray shape,
  ByVal locals, ByRef aliasing, and ByRef/writeback evidence;
- call-site descriptors consumed by VM, JIT, COM, native Declare, and exported
  callable paths.

The executable semantic package must carry both descriptor families. A declared
type descriptor without call/coercion semantics is not enough for JIT lowering.

## Expression Classification Model

Every expression node lowered into package metadata should have an expression
semantic descriptor:

```rust
pub struct ExpressionSemanticsDescriptor {
    pub expression_id: ExpressionId,
    pub source_span: Option<SourceSpanId>,
    pub classification: ExpressionClassification,
    pub declared_type: VbaTypeId,
    pub value_state_space: ValueStateSpace,
    pub carrier_hint: RuntimeCarrierKind,
    pub default_member_policy: DefaultMemberPolicy,
}

pub enum ExpressionClassification {
    Value,
    Variable,
    Property,
    Function,
    Subroutine,
    UnboundMember,
    ObjectMember,
    ArrayElement,
    UdtField,
    WithExpression,
    AddressOf,
}
```

Classification matters for assignment and calls. In particular, a ByRef
argument that is a variable has different aliasing behavior from a ByRef
argument whose expression is a value, function, property, or unbound member.
Object expressions can also trigger default-member binding before ordinary
Let-coercion.

## Coercion Model

OxVba needs explicit descriptors for both coercion families:

```rust
pub struct CoercionDescriptor {
    pub source_declared_type: VbaTypeId,
    pub source_value_state_space: ValueStateSpace,
    pub target_declared_type: VbaTypeId,
    pub kind: CoercionKind,
    pub static_status: CoercionStaticStatus,
    pub runtime_failure: RuntimeFailurePolicy,
    pub source_anchor: Option<SpecAnchorId>,
}

pub enum CoercionKind {
    Let,
    Set,
    ExplicitConversionIntrinsic,
    BoundaryProjection,
}

pub enum CoercionStaticStatus {
    Valid,
    Invalid,
    RequiresRuntimeCheck,
    ImplementationDefined,
}
```

Let-coercion covers ordinary value assignment and most non-object parameter
binding. It must include numeric, Boolean, Date, String, fixed-length String,
Byte array, non-Byte array, UDT, Error/CVErr, Null, Empty, Variant, class,
Object, and Nothing cases.

Set-coercion covers object-reference assignment. It must not be collapsed into
Let-coercion. `Set` assignment, `Property Set`, object parameters, `Nothing`,
class/interface conformance, and COM object projection all depend on this
separation.

The package must preserve whether coercion was checked statically, requires a
runtime helper, or is intentionally implementation-defined. A typed JIT fast
path may inline only rows whose coercion behavior is proved or guarded.

## Operator Semantics

Operators are not just primitive machine operations. Each operator needs a
descriptor that names operand coercion, result type, Null/Error propagation,
overflow/type-mismatch policy, string comparison mode, and helper requirements.

```rust
pub struct OperatorSemanticsDescriptor {
    pub operator: VbaOperator,
    pub left_declared_type: Option<VbaTypeId>,
    pub right_declared_type: Option<VbaTypeId>,
    pub result_declared_type: VbaTypeId,
    pub result_value_state_space: ValueStateSpace,
    pub coercion_policy: OperatorCoercionPolicy,
    pub error_policy: RuntimeFailurePolicy,
    pub compare_mode: Option<ModuleCompareMode>,
    pub helper: Option<HelperAbiId>,
}
```

Required operator families:

- arithmetic: unary `-`, `+`, binary `-`, `*`, `/`, `\`, `Mod`, `^`;
- concatenation: `&`;
- relational: `=`, `<>`, `<`, `>`, `<=`, `>=`;
- pattern/object: `Like`, `Is`, `TypeOf ... Is`;
- logical/bitwise: `Not`, `And`, `Or`, `Xor`, `Eqv`, `Imp`;
- assignment-adjacent string operations such as `Mid` statement semantics.

The `+` operator is a named risk surface because it can mean addition or string
concatenation depending on operand value types. The `&` operator is the forced
string-concatenation path. Relational string comparisons must preserve the
module `Option Compare` mode. `Is` is object identity, not value equality.

The first implementation step is not to hand-code the full table in the JIT.
The package must expose an operator descriptor or helper reference, and the VM
must be able to run fixtures that prove the descriptor matches current
interpreter behavior or identify a real VM limitation.

## Assignment And Property Semantics

Let assignment:

- may omit the `Let` keyword in source;
- uses Let-coercion from the expression value to the target declared type;
- handles assignment into variables, properties, array elements, UDT fields,
  and fixed-length strings;
- must preserve Null, Empty, Error/CVErr, and default-member behavior.

Set assignment:

- requires the `Set` keyword;
- uses Set-coercion to assign object references;
- handles `Nothing`, class/interface compatibility, `Object`, imported COM
  object/interface descriptors, and object lifetime.

Property descriptors must preserve:

- `Property Get`, `Property Let`, and `Property Set` kind;
- shared property name pairing rules;
- equivalent property parameter lists;
- value parameter declared type;
- the rule that the `Property Let` value type matches the corresponding
  `Property Get` return type;
- the rule that `Property Set` value type is Object, Variant, or a named class;
- the rule that a property-LHS value parameter has runtime ByVal semantics even
  when its source mechanism is omitted or written as ByRef.

Default-member binding is a distinct package fact. A Let-style context may need
to bind through an object's default value member, while a Set-style context
assigns the object reference itself.

## Call Binding Semantics

Every call site should lower to a call descriptor:

```rust
pub struct CallSiteDescriptor {
    pub call_site_id: CallSiteId,
    pub target: CallableTargetDescriptor,
    pub specified_arguments: Vec<ArgumentDescriptor>,
    pub resolved_bindings: Vec<ArgumentBindingDescriptor>,
    pub param_array_binding: Option<ParamArrayBindingDescriptor>,
    pub invocation_kind: InvocationKind,
    pub error_policy: RuntimeFailurePolicy,
}

pub struct ArgumentBindingDescriptor {
    pub parameter_index: usize,
    pub argument_index: Option<usize>,
    pub parameter_slot: Option<usize>,
    pub mechanism: ParameterMechanism,
    pub argument_classification: Option<ExpressionClassification>,
    pub declared_type_check: CallTypeCompatibility,
    pub coercion: Option<CoercionDescriptor>,
    pub storage: ArgumentStorageKind,
}

pub enum ArgumentStorageKind {
    DirectByRefAlias { source_slot: usize },
    LocalByValCopy,
    LocalByRefExpressionTemp,
    OptionalDefaultLocal,
    OptionalMissingState,
    ParamArrayElement,
    BoundaryProjectedTemp,
}
```

Static call compatibility must preserve the MS-VBAL mapping sequence:

- positional arguments map left-to-right;
- named arguments map by case-insensitive name value;
- duplicate mappings are incompatible;
- too many positional arguments are incompatible unless the last parameter is a
  `ParamArray`;
- any non-optional parameter without a mapped argument is incompatible;
- omitted positional values mapped to non-optional parameters are incompatible;
- ByVal non-object arguments require valid Let-coercion to the parameter type;
- ByVal object arguments must be Object, class, or Variant-compatible;
- ByRef non-object, non-Variant arguments require exact declared type match;
- ByRef Variant parameters require the MS-VBAL Variant-specific binding rule
  and must not be silently narrowed to a primitive exact-match rule;
- ByRef object arguments require object/class declared compatibility.

Runtime call binding must preserve:

- runtime error 450 for wrong argument count or invalid property assignment,
  except when a `ParamArray` consumes extra positional arguments;
- runtime error 448 for unknown named arguments, duplicate mappings, and
  omitted positional values mapped to non-optional parameters;
- runtime error 449 for unmapped non-optional parameters;
- ByVal parameters as procedure-local variables initialized from the mapped
  argument after the required coercion;
- ByRef variable arguments as aliases to the caller variable;
- ByRef value/function/property/unbound arguments as procedure-local temporary
  variables where the invocation rules allow that binding;
- unmapped optional parameters as procedure-local variables initialized from
  their default value, or as the explicit missing/default state required for
  Variant optional parameters;
- `ParamArray` as a new Variant array with lower bound 0, containing extra
  positional arguments, or an empty lower-0/upper--1 array when no extras are
  supplied.

COM and native Declare calls reuse this descriptor shape, but add boundary
projection and writeback descriptors. A COM/natively projected ByRef is not a
reason to weaken core VBA ByRef semantics; it is an external ABI projection
from the semantic binding.

## Method And Event Invocation

Method calls include an implicit current-object parameter:

- the implicit current object is ByVal;
- it has procedure extent;
- its declared type is the class containing the method;
- it is exposed through `Me`;
- it is not assignable by user code.

Events and event handlers must use the same signature compatibility model as
procedure invocation. Event declaration parameters describe the arguments
required for `RaiseEvent`; they are not ordinary variable bindings by
themselves.

Default function/subroutine invocation on objects must be represented as a
resolved call target, not as an untracked runtime fallback. Late-bound COM
default members must carry the same named/default argument facts used by the
COM descriptor.

## Package And VM/JIT Requirements

The executable semantic package must carry:

- expression descriptors for coercion-sensitive expression nodes;
- operator descriptors or helper IDs for every lowered operator;
- call-site descriptors with argument mapping, optional/default state,
  ParamArray shape, ByVal/ByRef storage, and diagnostics;
- property accessor descriptors with `Get`/`Let`/`Set` pairing and value-param
  semantics;
- default-member descriptors for object and COM member access;
- boundary projection descriptors for COM, native Declare, and exported
  callables.

The VM should consume these descriptors where they affect execution, and VM
snapshot evidence should report enough call/coercion metadata to prove the JIT
used the same semantic package.

The JIT may specialize only when:

- the relevant descriptor row is present in the package;
- the VM can run the same package path or the gap is classified explicitly;
- runtime helper/deopt paths preserve slot state, error state, object identity,
  cleanup, and ByRef alias/writeback behavior;
- unsupported shapes produce deterministic diagnostics rather than silent VM
  fallback.

## Spec Cross-Reference Review

Review result for this draft:

| Area | Foundation/MS spec anchor | Required OxVba package fact | Current risk |
|---|---|---|---|
| Declared types and non-types | MS-VBAL table 2/table 3, segments around `SEG-000632`, `SEG-000653`; type-system doc | `VbaTypeId`, value-state space, carrier | Current code still lacks one central package registry. |
| Let-coercion | MS-VBAL 5.5.1, spec items `00185..00201`; seed table [`../validation/VBA_COERCION_SEED_TABLE_V1.csv`](../validation/VBA_COERCION_SEED_TABLE_V1.csv) | `CoercionDescriptor { kind: Let }` | First helper-backed seed rows exist, but canonical descriptor ids and full truth table extraction remain incomplete. |
| Set-coercion | MS-VBAL 5.5.2, spec items `00202..00206` | `CoercionDescriptor { kind: Set }` | Object/class/interface/COM conformance must stay descriptor-backed. |
| Operators | MS-VBAL 5.6.9, spec items `00219..00246`; `Option Compare` segments `SEG-001565..001571`; seed table [`../validation/VBA_OPERATOR_SEED_TABLE_V1.csv`](../validation/VBA_OPERATOR_SEED_TABLE_V1.csv) | `OperatorSemanticsDescriptor` | First helper-backed seed rows exist, but canonical descriptor ids and full truth table extraction remain incomplete. |
| Object/member binding | MS-VBAL Set/default-member/property/event anchors; seed table [`../validation/VBA_OBJECT_MEMBER_BINDING_SEED_TABLE_V1.csv`](../validation/VBA_OBJECT_MEMBER_BINDING_SEED_TABLE_V1.csv) | `ObjectMemberBindingDescriptor` and property/default-member/event descriptors | First binding seed rows exist for `Set`, `Nothing`, properties, default members, class/interface routes, COM dispatch, early-bound COM, and events, but canonical descriptor ids and VM member-binding consumption remain incomplete. |
| Procedure signatures | MS-VBAL anchors `p:1418..1470`; `SEG-002047..002101` | `ProcedureSignatureDescriptor` and `ParameterDescriptor` | Existing metadata is useful but not full enough for package/JIT parity. |
| Property signatures | MS-VBAL anchors `p:1482..1488`; `SEG-002107..002121` | property pairing and value-param descriptor | Property value-param ByVal runtime semantics must be explicit. |
| Call-site mapping | MS-VBAL `SEG-002162..002189` | `CallSiteDescriptor` and `ArgumentBindingDescriptor` | ByRef alias/temp/default/ParamArray distinctions must not be inferred in JIT. |
| Event invocation | MS-VBAL `CONF-...-0178`, `SEG-002488` | event signature compatibility descriptor | Event params are signature facts, not ordinary locals. |
| COM projection | MS-OAUT VARIANT/BSTR/SAFEARRAY/IDispatch anchors listed above | boundary projection and helper ABI descriptors | COM wire descriptors must remain projections, not core type replacements. |

Open follow-up:

- expand the first machine-readable coercion seed table into the full MS-VBAL
  Let/Set truth table and bind rows to package `CoercionDescriptor` ids;
- expand the first operator seed table into a compact MS-VBAL truth table for
  numeric, string, Null, Empty, Error/CVErr, object, and Variant rows;
- expand the first object/member binding seed table into a package-owned
  member registry with canonical descriptor ids, property/default-member
  pairing, dispatch cache policy, event graph semantics, and VM evidence;
- audit current compiler/VM metadata against the signature descriptors in
  `VBA_TYPE_SYSTEM_V1.md`;
- extend VM-runnable fixtures beyond the VMR-04 seed, which now covers ByRef
  alias versus ByRef expression temp, Optional default metadata, Optional
  `Variant` missing-policy metadata, and empty/non-empty ParamArray shape;
- use the VMR-04 call-gap ledger in
  `EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md` before changing call
  execution. That ledger currently classifies ByRef expression no-writeback as
  oracle/test work, ByVal declared-type call-entry coercion as a VM call-binding
  limitation plus missing coercion metadata, and omitted Optional `Variant` as
  a VM/runtime value-state limitation;
- add remaining fixtures for `Property Let`/`Set` value-param behavior and
  object default-member binding.
