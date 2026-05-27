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
  `ParamArray`, named fixed arguments with positional ParamArray packs, and
  current compile diagnostics for named/arity/ParamArray rejection forms. The
  VMR-04 package fixtures now cover these as descriptor evidence, with ByVal
  declared-type call-entry coercion and Optional missing `Variant` explicitly
  classified in the completion-map VMR-04 call-gap ledger before
  behavior-changing follow-up.
- Lifecycle seed: primitive, `Variant`, Decimal-in-Variant payload, `BStr`,
  `SafeArray`, `ObjectRef`, UDT fields, ByRef temporaries, and COM/native
  boundary temporaries.
- Object/member seed: `Set` object assignment, `Nothing`, default member in
  `Let` context, `Property Get`/`Let`/`Set` shape, and late-bound dispatch
  descriptor.

## Coercion Seed Table v1

The first checked-in coercion seed table is:

[`../validation/VBA_COERCION_SEED_TABLE_V1.csv`](../validation/VBA_COERCION_SEED_TABLE_V1.csv)

This table is intentionally narrow. It records the current helper families that
VM execution already depends on:

- identity and selected numeric widening in `oxvba_runtime::coerce_to`;
- `Empty` to numeric/Boolean/string helper behavior;
- Boolean numeric coercion where `True` is `-1` and `False` is `0`;
- string/BSTR conversion via `variant_to_vba_string`;
- VM compatibility helpers for text, numeric, `i32`, and `f64` conversion;
- `Null` propagation and `CVErr` arithmetic errors in current VM arithmetic;
- truthiness used by VM branch predicates;
- `Decimal` as a Variant subtype/runtime carrier, including Decimal-to-`f64`
  compatibility and string display;
- the known VMR-04 call-entry gap where `ByVal Long` into a declared `Double`
  parameter is described by metadata but not yet applied at callee entry.

The table uses `metadata-missing` even when a helper is VM-backed because rows
are not yet canonical `CoercionDescriptor` package facts. The table also keeps
helper-specific behavior separate when the current implementation has multiple
conversion paths. For example, `variant_to_vba_string(Boolean)` returns
`True`/`False`, while `runtime_variant_to_text(Boolean)` returns `-1`/`0` for
several current VM helpers. A future descriptor-backed coercion pass must decide
which source language context owns each path before routing execution through
the table.

## Operator Seed Table v1

The first checked-in operator and branch predicate seed table is:

[`../validation/VBA_OPERATOR_SEED_TABLE_V1.csv`](../validation/VBA_OPERATOR_SEED_TABLE_V1.csv)

This table records the current VM helper families for:

- `+` over current integer-compatible, floating-compatible, Boolean, `Empty`,
  numeric-string, and string/string paths;
- `-`, `*`, `/`, `\`, `Mod`, `^`, and unary `-` over the current numeric
  compatibility helpers;
- forced concatenation with `&`, including the current helper's Null-as-empty
  and text-conversion-error-to-empty behavior;
- string and numeric comparisons under `StringCompareMode::Binary` and
  `StringCompareMode::Text`;
- `Null` and `Empty` comparison behavior in the current VM;
- truthiness, `Not`/`And`/`Or`, and `JumpIfZero` branch predicates;
- internal `i32` fast paths, explicitly marked as implementation fast paths
  and not semantic proof.

As with coercion, these rows are not canonical package descriptors yet. They are
current helper evidence and a guardrail for future table-backed VM behavior or
JIT lowering. Rows that look suspicious from a VBA compatibility perspective,
such as `Null` comparison producing a deterministic false Boolean and
`&` swallowing text conversion failures as empty text, remain classified as
current helper behavior until a spec/oracle-backed behavior bead changes them.
The `Option Compare Text` seed is also current-helper evidence only: the VM
normalizes text with ASCII lowercasing today, so full VBA locale/collation parity
still needs table and oracle coverage.

## Lifecycle Cleanup Seed Table v1

The first checked-in lifecycle and cleanup seed table is:

[`../validation/VBA_LIFECYCLE_CLEANUP_SEED_TABLE_V1.csv`](../validation/VBA_LIFECYCLE_CLEANUP_SEED_TABLE_V1.csv)

This table records current runtime and VM evidence for:

- primitive declared slots that have no owned cleanup;
- declared `Variant` slots, including Decimal as a Variant payload rather than
  ordinary declared storage;
- variable and fixed-length string cleanup obligations;
- dynamic arrays, SAFEARRAY element ownership, and `ParamArray` packs;
- `ObjectRef` AddRef/Release ownership and object identity;
- UDT primitive and owning fields, fixed strings, and fixed arrays;
- ByRef alias/writeback and ByRef expression temporary policies;
- COM dispatch temporaries, native Declare writeback buffers, and future deopt
  cleanup materialization.

Rows are current-state seed evidence, not package-owned lifecycle descriptors.
The table deliberately keeps `metadata-missing`, `test-shortcoming`,
`VM-limitation`, `interop-limitation`, and `oracle-required` gaps visible until
VM evidence records lifecycle observations and descriptor-backed cleanup maps
exist. A later behavior-changing VM or JIT path may use these rows only after
the row has a canonical descriptor id and its branch, return, error, helper
failure, boundary, and deopt cleanup obligations are fixture-backed.

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
