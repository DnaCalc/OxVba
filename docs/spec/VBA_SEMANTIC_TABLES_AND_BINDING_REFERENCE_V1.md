# VBA Semantic Tables And Binding Reference v1

Status: `working-draft`
Date: 2026-05-26
Scope owner: OxVBA compiler/VM/runtime/COM/native-readiness
Companion semantics:
[`VBA_TYPE_SYSTEM_V1.md`](VBA_TYPE_SYSTEM_V1.md),
[`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md)

## Purpose

Define the machine-readable semantic tables and binding audits needed to make
bytecode plus metadata executable enough for the VM and future JIT. This single
reference covers the narrower topics that would otherwise become several small
docs: coercion/operator truth tables, call-site descriptor audit, slot lifecycle
and cleanup, and object/member binding.

## Source Anchors

Canonical source material is listed in
[`../FOUNDATION_SPEC_REFERENCE.md`](../FOUNDATION_SPEC_REFERENCE.md).

Initial extraction anchors:

- MS-VBAL Let-coercion: `SPEC-discovered-ms-vbal-250520-f945507e-00185..00201`
- MS-VBAL Set-coercion: `SPEC-discovered-ms-vbal-250520-f945507e-00202..00206`
- MS-VBAL operators: `SPEC-discovered-ms-vbal-250520-f945507e-00219..00246`
- MS-VBAL procedure/call binding: `SEG-002047..002121`,
  `SEG-002162..002189`
- MS-VBAL arrays/ParamArray: `SEG-001689..001691`, `SEG-002101`,
  `SEG-002180..002182`
- MS-OAUT COM projection: `CONF-discovered-ms-oaut-240423-b76f9b41-0010..0016`,
  `0023..0028`, `0042`, `0050..0077`, `0173..0188`, `0202`, `0224`,
  `0227`, `0278..0287`

## Coercion Table Shape

Let and Set coercion rows should be extracted into a table shaped like:

```text
coercion_id
kind
source_declared_type
source_value_state
target_declared_type
static_status
runtime_result
runtime_error
helper_id
spec_anchor
oracle_anchor
current_vm_status
```

Required families:

- numeric-to-numeric, including `Currency`, `Date`, Boolean numeric truth, and
  Decimal-in-Variant;
- Boolean, Date, String, fixed-length String, Byte array, non-Byte array, UDT,
  Error/CVErr, Null, Empty, Variant;
- class/Object/Nothing Let and Set behavior;
- COM/native boundary projection as separate rows from core VBA coercion.

## Operator Table Shape

Operator rows should be table-driven before any direct typed execution path is
accepted:

```text
operator_id
operator
left_declared_type
left_value_state
right_declared_type
right_value_state
compare_mode
result_declared_type
result_value_state
runtime_error
helper_id
spec_anchor
oracle_anchor
current_vm_status
```

Required families:

- arithmetic: unary `-`, `+`, binary `-`, `*`, `/`, `\`, `Mod`, `^`;
- forced concatenation: `&`;
- ambiguous addition/concatenation: `+`;
- relational and string comparison under `Option Compare`;
- `Like`, `Is`, `TypeOf ... Is`;
- logical/bitwise: `Not`, `And`, `Or`, `Xor`, `Eqv`, `Imp`;
- Null, Empty, Error/CVErr, object, and Variant edge rows.

## Call-Site Descriptor Audit

Audit current compiler and VM metadata against this target:

```text
procedure_id
call_site_id
target_kind
target_signature_id
argument_order
named_argument_map
omitted_argument_map
optional_default_policy
param_array_policy
byval_copy_slots
byref_alias_sources
byref_temp_slots
writeback_policy
default_member_policy
error_policy
```

The first audit fixtures should cover:

- ByRef variable aliasing;
- ByRef expression/property/function temporary behavior;
- ByVal coercion;
- omitted optional arguments with and without defaults;
- Variant optional missing/error state;
- empty and non-empty ParamArray shape;
- named arguments, duplicate names, unknown names, too many args, missing
  required args;
- `Property Let` value-param ByVal semantics;
- `Property Set` object assignment;
- object default-member invocation.

## Slot Lifecycle And Cleanup

Every slot or temporary that can own runtime state needs lifecycle metadata:

```text
lifecycle_id
slot_or_temp_kind
declared_type
carrier
init_policy
copy_policy
assign_policy
drop_policy
error_exit_policy
deopt_policy
byref_policy
helper_cleanup_policy
```

Required carrier families:

- primitive scalars;
- declared `Variant`, including Decimal/Error/Null/Empty payload states;
- `BStr`, including fixed-length string assignment;
- `SafeArray`, including element ownership;
- `ObjectRef`, class/interface/COM object identity;
- UDT fields, nested UDTs, fixed strings, fixed arrays;
- COM/native marshalled temporaries and writeback buffers.

## Object And Member Binding

Object/member binding needs a package-level descriptor, not runtime-only lookup:

```text
binding_id
target_declared_type
member_name
member_kind
default_member
property_accessor_group
dispatch_kind
argument_binding_policy
result_declared_type
object_identity_policy
cache_invalidation_policy
fallback_or_unsupported_policy
```

Required rows:

- class member calls and property access;
- interface implementation member mapping;
- `Object` late-bound member access;
- default member binding in Let contexts;
- `Set` contexts that assign the object reference itself;
- imported COM class/interface members;
- late-bound COM `IDispatch::Invoke` named/default argument mapping;
- early-bound COM dispatch/vtable strategy;
- event source and `WithEvents` handler binding.

## Seed Table Targets For VM Rework

The first extraction pass should produce small, runnable seed sets rather than
attempting full table coverage at once:

- Coercion seed: `Empty`, `Null`, `Error`/`CVErr`, Boolean, `Long`, `Double`,
  `String`, and `Variant` rows used by current helpers.
- Operator seed: `+`, `&`, comparison, truthiness, and branch predicates over
  `Long`, `Double`, `String`, Boolean, `Variant`, `Null`, and `Empty`.
- Call seed: ByVal scalar, ByRef scalar alias, ByRef expression temporary,
  Optional with default, Optional missing `Variant`, and empty/non-empty
  `ParamArray`.
- Lifecycle seed: primitive, `Variant`, `BStr`, `SafeArray`, `ObjectRef`, and
  UDTs containing primitive, `String`, and `Variant` fields.
- Object/member seed: `Set` object assignment, `Nothing`, default member in
  `Let` context, `Property Get`/`Let`/`Set` shape, and late-bound dispatch
  descriptor.

## Minimum Table Artifact

Before full CSV automation, each seed table may be represented as a checked-in
markdown table or embedded fixture manifest. The minimum useful row fields are:

- stable row id;
- source/spec anchor and optional Office oracle anchor;
- current VM status;
- expected evidence fields;
- gap classification;
- owning descriptor family.

## Output Rule

These tables are package inputs, not test decorations. A bytecode or JIT path
may use a fast typed operation only when the corresponding row exists, is
descriptor-backed, and has VM evidence or an explicit oracle/deferred status.
