# VBA Semantic Tables And Binding Reference v1

Status: current VBA semantic/evidence reference; implementation state tracked in canonical matrices
Date: 2026-05-26
Authority review: 2026-07-11
Scope owner: semantic table shapes and authority/evidence routing
System clauses: `AUTH-SPEC-001`, `COMP-BIND-001`, `CONF-MATRIX-001`
Companion semantics:
[`VBA_TYPE_SYSTEM_V1.md`](VBA_TYPE_SYSTEM_V1.md),
[`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md)

## Purpose

Define the machine-readable semantic tables and binding audits needed to make
OxIR plus metadata executable enough for VM3 and the JIT. This single
reference covers the narrower topics that would otherwise become several small
docs: coercion/operator truth tables, call-site descriptor audit, slot lifecycle
and cleanup, and object/member binding.

Authority follows `CHARTER.md`, `OPERATIONS.md`, and
[`OXVBA_SYSTEM_CONTRACT_V1.md`](OXVBA_SYSTEM_CONTRACT_V1.md). Public
specifications and reproducible black-box Excel/VBA observations decide each
semantic row. Current compiler, helper, VM3, JIT, seed-table, or historical
package behavior is evidence or a divergence only; it cannot supply the
expected result without that authority. Unknown behavior remains an exact open
canonical row with a spec/oracle owner.

The text and code-like table shapes below are non-normative illustrations of
semantic and evidence fields. They do not prescribe Rust DTOs, artifact layout,
backend storage, or Windows transport. Active subsystem contracts own those
representations.

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
vm3_state
jit_state
evidence_state
residual_owner
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
vm3_state
jit_state
evidence_state
residual_owner
```

Required families:

- arithmetic: unary `-`, `+`, binary `-`, `*`, `/`, `\`, `Mod`, `^`;
- forced concatenation: `&`;
- ambiguous addition/concatenation: `+`;
- relational and string comparison under `Option Compare`;
- `Like`, `Is`, `TypeOf ... Is`;
- logical/bitwise: `Not`, `And`, `Or`, `Xor`, `Eqv`, `Imp`;
- Null, Empty, Error/CVErr, object, and Variant edge rows.

## Call-Site Semantic Audit

Every call-site coverage row must distinguish at least:

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

Required coverage includes:

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

Object/member binding must be resolved as a compiler-owned semantic fact, not
invented by a backend-local runtime lookup:

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
unsupported_policy
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

## Seed Table Authority And Divergence Disposition

The following checked-in seed tables are historical coverage inventories, not
canonical implementation or behavior truth:

- [`VBA_COERCION_SEED_TABLE_V1.csv`](../validation/VBA_COERCION_SEED_TABLE_V1.csv)
  covers identity, numeric/Boolean/String conversions, `Empty`, `Null`,
  `Error`/`CVErr`, Decimal-in-Variant, and selected call-entry coercions;
- [`VBA_OPERATOR_SEED_TABLE_V1.csv`](../validation/VBA_OPERATOR_SEED_TABLE_V1.csv)
  covers arithmetic, `+`, `&`, comparisons, truthiness, branch predicates,
  Null/Empty/Error states, and `Option Compare`;
- [`VBA_LIFECYCLE_CLEANUP_SEED_TABLE_V1.csv`](../validation/VBA_LIFECYCLE_CLEANUP_SEED_TABLE_V1.csv)
  covers scalar, Variant, BSTR, SAFEARRAY, ObjectRef, UDT, ByRef temporary, and
  external-boundary cleanup obligations;
- [`VBA_OBJECT_MEMBER_BINDING_SEED_TABLE_V1.csv`](../validation/VBA_OBJECT_MEMBER_BINDING_SEED_TABLE_V1.csv)
  covers `Set`/`Nothing`, property groups, default members, class/interface
  routing, COM member projection, events, and `WithEvents`.

Old helper names, `OxBundle` descriptors, VMR milestones, and VM/package tokens
in those files describe superseded implementation observations. They receive no
current completion credit and must not be copied into expected-result columns.
Useful cases are replayed on the current compiler -> verified OxIR/OxImage ->
VM3/JIT route and mapped to canonical rows.

Several historical observations are especially important divergence inputs:

- Boolean string conversion was observed through helpers with different
  `True`/`False` versus `-1`/`0` results;
- `Null` comparison was observed producing a deterministic false Boolean;
- `&` was observed swallowing some text-conversion failures as empty text;
- `Option Compare Text` was observed using ASCII-only lowercasing;
- selected coercion, lifecycle, property/default-member, array/UDT, and COM
  routes covered only bounded shapes.

None of those observations is the VBA target by default. Each must be decided
from a public specification or reproducible Excel/VBA observation, then owned
by the exact Core typed-binding/runtime/differential/oracle row or by the
relevant Windows interop row. Until that happens, the row remains open and a
VM3/JIT specialization may not rely on the historical result.

## Minimum Semantic Row

Whatever canonical representation an owning workset selects, a useful semantic
row distinguishes:

- stable row id;
- public-spec anchor and, when needed, reproducible Office oracle anchor;
- expected result/error and relevant side-effect, lifecycle, transport, and
  balance observables;
- VM3 and JIT evidence states without treating either as authority;
- residual classification and active owner;
- the semantic fact family consumed by the current subsystem contracts.

## Output Rule

These tables are semantic inputs, not test decorations or DTO specifications. A
VM3 or JIT path may use a fast typed operation only when the corresponding
canonical row is authoritative and verified and the active compiler/artifact
contracts preserve the required facts. Deferred or historical evidence cannot
authorize the path.
