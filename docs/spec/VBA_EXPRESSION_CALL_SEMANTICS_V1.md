# VBA Expression And Call Semantics v1

Status: current VBA semantic reference; implementation and evidence tracked in canonical matrices
Date: 2026-05-26
Authority review: 2026-07-11
Scope owner: VBA expression, assignment, property, and call semantics
System clauses: `AUTH-SPEC-001`, `COMP-BIND-001`, `RUNTIME-EVAL-001`

## Purpose

Define the expression, coercion, assignment, and procedure-invocation semantics
that sit beside the declared type model in
[`VBA_TYPE_SYSTEM_V1.md`](VBA_TYPE_SYSTEM_V1.md).

The type-system reference answers "what type is declared here?" This document
answers "what happens when values, expressions, properties, and calls are
evaluated?" Both documents are required inputs to compiler analysis, OxIR/OxImage,
VM3/JIT evidence, backend lowering plans and COM/native descriptors.

This is not a JIT-only contract. Coercion, operator, call-site, default-member,
and ByRef meaning are compiler-owned semantic facts shared by all consumers.
Their concrete representation belongs to the active compiler and OxIR/OxImage
contracts.

Authority follows `CHARTER.md`, `OPERATIONS.md`, and
[`OXVBA_SYSTEM_CONTRACT_V1.md`](OXVBA_SYSTEM_CONTRACT_V1.md). Public
specifications and reproducible black-box Excel/VBA observations decide
behavior. Current helper, VM3, JIT, or historical package observations are
evidence or divergences only; they never become provisional expected behavior
without that authority. Uncertainty remains an exact canonical spec/oracle row
with an active owner.

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

Rust-like structures below are non-normative illustrations of facts that must
remain distinguishable. They do not prescribe DTO names, crate APIs, OxImage
layout, VM frames, JIT ABI, or Windows transport structures.

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

The compiler fact set and verified executable semantics must preserve both
fact families. A declared type without call/coercion meaning is insufficient
for any backend.

## Expression Classification Model

Every relevant expression must retain the following semantic facts; this shape
is illustrative:

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

The semantic facts must preserve whether coercion was checked statically, requires a
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

An operator fast path is admissible only when its semantic row is authoritative
and verified. VM3/JIT fixtures test that shared meaning against public
specification or Excel/VBA evidence; current interpreter behavior cannot
self-authorize the expected result.

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

Default-member binding is a distinct semantic fact. A Let-style context may need
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

## Consumer Fact Obligations

The following semantic distinctions must survive into verified execution:

- expression descriptors for coercion-sensitive expression nodes;
- operator descriptors or helper IDs for every lowered operator;
- call-site descriptors with argument mapping, optional/default state,
  ParamArray shape, ByVal/ByRef storage, and diagnostics;
- property accessor descriptors with `Get`/`Let`/`Set` pairing and value-param
  semantics;
- default-member descriptors for object and COM member access;
- boundary projection descriptors for COM, native Declare, and exported
  callables.

The active compiler and OxIR/OxImage contracts own their representation. VM3
and the JIT consume the same verified meaning, and differential evidence must
show that both used the same call/coercion decisions.

The JIT may specialize only when:

- the relevant semantic row and verified facts are present;
- VM3 admits and executes the same declared capability row;
- runtime helper and fault paths preserve slot state, error state, object identity,
  cleanup, and ByRef alias/writeback behavior;
- unsupported target shapes are rejected before partial code generation rather
  than silently falling back to VM3.

## Authority Cross-Reference And Closure Routing

| Area | Public authority anchor | Required semantic distinction | Canonical closure route |
|---|---|---|---|
| Declared types and value states | MS-VBAL table 2/table 3 around `SEG-000632`, `SEG-000653` | declared type, value-state space, carrier family | `CORE-READINESS/CORE-TYPED-BINDING` plus verified artifact rows |
| Let/Set coercion | MS-VBAL 5.5.1/5.5.2, items `00185..00206` | coercion family, static legality, runtime result/error | Core typed-binding and structural differential/oracle rows |
| Operators | MS-VBAL 5.6.9, items `00219..00246`; `Option Compare` `SEG-001565..001571` | operand/result types, Null/Error propagation, comparison mode | Core runtime-eval, VM3/JIT, and Excel-oracle rows |
| Object/member binding | MS-VBAL Set/default-member/property/event anchors | resolved target, accessor/default-member choice, identity and call facts | Core typed-binding; COM transport remains in Windows rows |
| Procedures and properties | MS-VBAL `SEG-002047..002121` | exact signature, accessor pairing, property value-parameter semantics | `CORE-READINESS/CORE-TYPED-BINDING` |
| Call-site mapping | MS-VBAL `SEG-002162..002189` | ByRef alias/temp, Optional/default, ParamArray, named/omitted mapping | Core typed-binding, OxIR call/ByRef, and differential rows |
| Event invocation | MS-VBAL `CONF-...-0178`, `SEG-002488` | event signature compatibility and argument mapping | Core compiler/VM rows; native event transport in Windows rows |
| COM projection | MS-OAUT anchors listed above | boundary projection without replacing core semantics | Windows metadata, interop-plan, COM client/server/event rows |

The seed coercion, operator, lifecycle, and object/member tables linked from
[`VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md`](VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md)
are historical coverage inputs. Their prior helper, `OxBundle`, VM, or VMR
observations receive no present implementation or expected-behavior credit.
Any useful case must be replayed through the current stack and decided by its
public authority or a reproducible Excel/VBA oracle capture. An uncovered case
is split into the exact canonical row and owner above before this semantic
surface can be called complete.
