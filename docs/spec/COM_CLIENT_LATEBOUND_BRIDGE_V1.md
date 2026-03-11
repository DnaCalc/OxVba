# COM_CLIENT_LATEBOUND_BRIDGE_V1

Status: `working-draft`  
Date: 2026-03-05  
Scope slice: `v393..v400`

## Goal

Define the executable bridge from VBA late-bound call semantics to HAL COM transport without ambiguity.

## Bridge Layers

1. VBA source semantics
- Surface forms:
  - `CreateObject(<selector>)`
  - late-bound member invocation (currently explicit `DispatchInvoke` transport in executable subset)

2. Compiler/VM transport
- `IntrinsicCreateObjectHost { prog_id }`
- `IntrinsicDispatchInvokeHost { object, member, args }`
- invoke-arg transport shape:
  - each invoke argument now carries:
    - `slot` (or omission)
    - optional forwarded COM argument name metadata
- Known-literal lowering subset:
  - `CreateObject("Scripting.Dictionary") -> prog_id_token=4`
  - `CreateObject("OxVba.TestDispatch") -> prog_id_token=4` (controlled test lane alias)
  - `DispatchInvoke(..., "Count", ...) -> member_token=1`
  - `DispatchInvoke(..., "Exists", ...) -> member_token=2`
- `DispatchInvoke` arity subset:
  - 2-arg form (`object, member`) for no-arg member/property-get lanes
  - variadic form (`object, member, arg1, arg2, ...`)
  - each call argument is marshaled independently; a VBA array passed as one argument remains one marshaled argument token.

3. HAL COM transport
- `create_object(prog_id_token) -> object_token`
- `dispatch_invoke_v2(request) -> result_token`
- legacy scalar `dispatch_invoke(object_token, member_token, arg_token)` remains as a compatibility shim over `dispatch_invoke_v2`.
- request argument shape:
  - `ComInvokeArg { value, name }`
  - omission metadata survives the VM/HAL boundary
  - named-argument metadata survives the VM/HAL boundary for member-known dispatch lanes

4. Native COM adapter (Windows host-backed mode)
- Activation: `CLSIDFromProgID` + `CoCreateInstance`
- Invoke: `IDispatch::Invoke`
- Controlled test lane: `CreateObject("OxVba.TestDispatch")` activates an in-process OxVba-owned `IDispatch` object for deterministic integration testing without external COM registration dependencies.

## Contract Invariants

1. Object token identity is stable for the lifetime of the HAL binding.
2. Native dispatch pointer ownership is bound to COM-state lifecycle, not per-invoke temporary activation.
3. Unsupported or denied paths remain deterministic and must not mutate COM-state.
4. Failure translation is deterministic and tagged by the COM error taxonomy table.
5. Member-name lanes must keep deterministic per-object cache semantics for resolved DISPIDs on native Windows path.
6. Omitted argument packs in `DispatchInvoke`:
- property-get/no-arg member lanes may proceed with an empty argument vector;
- argument-required member lanes must fail deterministically.
7. Named-argument transport:
- member-known method/property-get lanes must preserve forwarded argument names through bytecode, VM, and HAL request transport;
- member-known property-put/property-putref lanes must canonicalize named/indexed arguments so the property value uses `DISPID_PROPERTYPUT`/`DISPID_PROPERTYPUTREF` without depending on caller argument order;
- default-member/direct-DISPID dispatch must not silently erase named arguments when runtime member identity is unresolved.
8. Variant token transport:
- the controlled and native Windows late-bound lane must preserve stable runtime token meaning for `VT_EMPTY`, `VT_NULL`, `VT_BOOL`, `VT_I4`, `VT_UI4`, and `VT_ERROR`;
- outbound invoke marshalling must emit `VT_NULL` and `VT_ERROR` when runtime null/error-tag values are supplied;
- unsupported `VARIANT` shapes must fail deterministically and must not silently coerce into incorrect integer tokens.
9. Event-trigger projection consumes the same authoritative argument vector used for invoke, and only synthesizes fallback payload shape when a native callback path is unavailable.

## Deferred Extensions

- Natural VBA member syntax to late-bound dispatch lowering.
- Default-member named-dispatch parity once runtime member identity is authoritative.
- Broad `VARIANT`/object/`SAFEARRAY` parity plus full `Invoke` result/error fidelity (`ArgErr`, `ExcepInfo`, `VarResult`).
- Full generic ProgID/member-name text selector path through current integer-token VM boundary.
