# JIT v2 Helper ABI Catalog v1

Status: `planning-abi`
Date: 2026-05-26
Owning workset:
[`../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md`](../worksets/WORKSET_2026-05-26_JIT_V2_CRANELIFT_PLANNING_STAGE.md)
Semantic contract:
[`JIT_V2_SEMANTIC_CONTRACT_AND_FACT_PACK_V1.md`](JIT_V2_SEMANTIC_CONTRACT_AND_FACT_PACK_V1.md)
Expression/call semantics:
[`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md)

## Purpose

Define the first helper ABI catalog for JIT v2. The catalog is intentionally
helper-first: correctness comes from shared runtime helpers and VM/JIT
differential evidence before specialization.

## ABI Version

Initial version: `jit-helper-abi-v1`.

All compiled procedures record the helper ABI version. A mismatch between the
compiled procedure and runtime helper table invalidates the compiled procedure.

## Common ABI Shape

Generated code calls helpers through registered symbols only. Ambient platform
symbol lookup is forbidden.

Recommended common shape:

```rust
extern "C" fn(
    vmctx: *mut JitVmContext,
    frame: *mut JitFrame,
    args: *const HelperArg,
    arg_len: usize,
    result: *mut HelperResult,
) -> JitStatus
```

Helpers must not unwind across generated code. Panic or invariant failure must
be translated to `JitStatus::HelperFault` or `JitStatus::InvariantViolation`
with diagnostic detail stored in `JitVmContext`.

## Descriptor Fields

Every helper descriptor records:

- symbol;
- category;
- ABI version;
- argument descriptors;
- result descriptor;
- slot reads and writes;
- ownership transfer;
- may allocate;
- may mutate frame;
- may set `Err`;
- may route runtime error;
- may call host services;
- may call COM/native code;
- may reenter OxVba;
- cleanup obligations;
- safepoint requirement;
- deopt snapshot requirement.

## Helper Arguments

`HelperArg` should support:

- slot index;
- slot carrier/layout id;
- immediate i32/i64/f64/bool;
- operation code;
- descriptor id;
- coercion/operator/call-site descriptor id;
- UDT descriptor id;
- UDT field id;
- bytecode PC;
- source map id;
- argument vector offset/count;
- ByRef writeback id.

Slots refer to declared carrier cells in `JitFrame`. Helpers should not take or
return `Variant` by value across the generated-code ABI. Declared `Variant`
slots and COM VARIANT projections use `VariantComLayout`; primitive and UDT
slots keep typed carrier/layout descriptors.

## Core Helper Catalog

| Symbol | Category | Purpose | Required for |
|---|---|---|---|
| `jitv2_error_route` | error | Route runtime/helper errors through VM-equivalent `Err` state. | TB03, all fallible helpers |
| `jitv2_err_number` | error | Load `Err.Number` into a declared destination slot. | TB03 |
| `jitv2_err_clear` | error | Clear `Err` state. | error fixtures |
| `jitv2_primitive_binop` | primitive arithmetic/coercion | Declared primitive `Add/Sub/Mul/Div/Cmp/Bool` operations by op code. | TB01 |
| `jitv2_primitive_truthy` | primitive control/coercion | Compute truthiness for declared primitive branch lowering. | TB01 |
| `jitv2_variant_binop` | declared Variant arithmetic/coercion | VM-equivalent Variant/dynamic operations by op code. | Variant-specific future bullets |
| `jitv2_carrier_copy` | carrier | Copy declared slot carriers with correct ownership. | all bullets |
| `jitv2_udt_copy` | UDT | Copy a descriptor-backed UDT aggregate. | TB02 |
| `jitv2_udt_field_load` | UDT | Load a field through verifier-checked UDT descriptor and field carrier. | TB02 |
| `jitv2_udt_field_store` | UDT | Store a field through verifier-checked UDT descriptor and field carrier. | TB02 |
| `jitv2_bstr_concat` | BSTR | Concatenate string operands into destination slot. | TB04 |
| `jitv2_bstr_len` | BSTR | Compute `Len` semantics for declared string carrier. | TB04 |
| `jitv2_bstr_release_temp` | cleanup | Release/clear temporary BSTR cleanup entry. | TB04 |
| `jitv2_array_set` | SAFEARRAY | Store element through runtime SAFEARRAY semantics. | TB05 |
| `jitv2_array_get` | SAFEARRAY | Load element through runtime SAFEARRAY semantics. | TB05 |
| `jitv2_array_foreach_init` | SAFEARRAY | Initialize For Each iterator over runtime array. | TB05 |
| `jitv2_array_foreach_next` | SAFEARRAY | Advance For Each and materialize next element. | TB05 |
| `jitv2_array_bounds` | SAFEARRAY | Load LBound/UBound metadata. | TB05 |
| `jitv2_com_create_object` | COM late-bound | Host-policy-aware `CreateObject` projection. | TB06 |
| `jitv2_com_dispatch_invoke` | COM late-bound | Descriptor-backed `IDispatch::Invoke` equivalent. | TB06 |
| `jitv2_com_early_invoke` | COM early-bound | Typelib-backed dispatch/vtable helper. | TB07 |
| `jitv2_native_invoke` | native Declare | Descriptor-backed native call and writeback helper. | TB08 |
| `jitv2_export_inbound_project` | exported callable | Project inbound ABI args into frame slots. | TB09 |
| `jitv2_export_outbound_project` | exported callable | Project return and ByRef writebacks out of frame. | TB09 |
| `jitv2_deopt_snapshot` | deopt | Materialize VM-resumable snapshot from frame. | all guarded paths |
| `jitv2_trace_event` | diagnostics | Emit compile/run/helper/deopt diagnostic events. | all bullets |

## Category Rules

### Error Helpers

Error helpers may mutate `Err` state and current PC. They must declare the
bytecode PC being routed. They may not mutate unrelated slots except the
declared `Err` destination slots.

### Primitive And Variant Arithmetic/Coercion Helpers

Primitive arithmetic helpers take destination, declared carrier, left/right slot
or immediate descriptors, operation code, and bytecode PC. Overflow, division,
truthiness, and conversion behavior must match VM semantics for the declared
types.

Declared `Variant` arithmetic helpers are separate. They handle Variant/dynamic
coercion, `Null`, `Error`, and COM VARIANT-compatible payloads without making
primitive locals use a Variant-only path.

### UDT Helpers

UDT helpers take a UDT descriptor id plus field ids or aggregate slots. They
must validate descriptor identity, field bounds, field carrier kinds, whole-copy
semantics, and cleanup obligations for owning fields.

### BSTR Helpers

BSTR helpers may allocate and must register temporary ownership when the result
is not immediately committed to a retained destination slot. Branch, return,
failure, and deopt paths must release or transfer cleanup entries.

### SAFEARRAY Helpers

SAFEARRAY helpers may allocate and may borrow element carriers. For Each helper
state must live in `JitFrame` so deopt can reconstruct iterator progress or
request deterministic unsupported behavior if reconstruction is not yet
implemented.

### COM Helpers

COM helpers may call host services, allocate boundary temporaries, set `Err`,
and reenter OxVba through events/callbacks in later slices. TB06/TB07 helpers
must record HRESULT, EXCEPINFO, ArgErr, selector, arity, named arguments,
descriptor digest, and object identity evidence.

### Native Declare Helpers

Native helpers may call host services and native code. They must use shared ABI
descriptors, not ad hoc call lowering. Writeback descriptors must record source
slot, projected storage, writeback kind, commit/cancel policy, and cleanup.
The current VM seed for TB08 covers the implemented native descriptor subset:
scalars, BSTR/string pointers, SAFEARRAY byte-buffer pointers, Variant cell
pointers, and scalar ByRef writeback. General Automation `Variant` and
`SAFEARRAY` declared-parameter ABI support is a future helper/descriptor
extension, not current VM behavior.

### Export Helpers

Export helpers project inbound ABI arguments into retained frame slots and
project return/writeback state out. Unsupported inbound shape is a diagnostic,
not VM fallback.

### Deopt Helpers

Deopt helpers record procedure id, bytecode PC, slot map, error state, cleanup
state, byref writebacks, COM/native boundary state, and host policy identity.

## First-Bullet Helper Coverage

| Tracer | Required helpers |
|---|---|
| TB01 | `jitv2_primitive_binop`, `jitv2_primitive_truthy`, `jitv2_carrier_copy`, `jitv2_deopt_snapshot`, `jitv2_trace_event` |
| TB02 | `jitv2_udt_copy`, `jitv2_udt_field_load`, `jitv2_udt_field_store`, `jitv2_carrier_copy`, `jitv2_deopt_snapshot`, `jitv2_trace_event` |
| TB03 | `jitv2_primitive_binop`, `jitv2_error_route`, `jitv2_err_number`, `jitv2_deopt_snapshot`, `jitv2_trace_event` |
| TB04 | `jitv2_bstr_concat`, `jitv2_bstr_len`, `jitv2_bstr_release_temp`, `jitv2_deopt_snapshot`, `jitv2_trace_event` |
| TB05 | `jitv2_array_set`, `jitv2_array_foreach_init`, `jitv2_array_foreach_next`, `jitv2_array_get`, `jitv2_array_bounds`, `jitv2_error_route`, `jitv2_deopt_snapshot`, `jitv2_trace_event` |
| TB06 | `jitv2_com_create_object`, `jitv2_com_dispatch_invoke`, `jitv2_error_route`, `jitv2_deopt_snapshot`, `jitv2_trace_event` |
| TB07 | `jitv2_com_create_object`, `jitv2_com_early_invoke`, `jitv2_error_route`, `jitv2_deopt_snapshot`, `jitv2_trace_event` |
| TB08 | `jitv2_native_invoke`, `jitv2_error_route`, `jitv2_deopt_snapshot`, `jitv2_trace_event` |
| TB09 | `jitv2_export_inbound_project`, `jitv2_export_outbound_project`, `jitv2_error_route`, `jitv2_deopt_snapshot`, `jitv2_trace_event` |

## Acceptance Gate

Before implementation starts, every helper used by a tracer bullet must have:

- descriptor entry;
- runtime owner crate named;
- VM source behavior identified;
- test evidence requirement named;
- cleanup and deopt policy named;
- unsupported behavior named.
