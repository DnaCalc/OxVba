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
- `IntrinsicDispatchInvokeHost { object, member, arg }`
- Known-literal lowering subset:
  - `CreateObject("Scripting.Dictionary") -> prog_id_token=4`
  - `DispatchInvoke(..., "Count", ...) -> member_token=1`
  - `DispatchInvoke(..., "Exists", ...) -> member_token=2`
- `DispatchInvoke` arity subset:
  - 3-arg form (`object, member, arg`)
  - 2-arg form (`object, member`) lowered with missing-arg sentinel token.

3. HAL COM transport
- `create_object(prog_id_token) -> object_token`
- `dispatch_invoke(object_token, member_token, arg_token) -> result_token`

4. Native COM adapter (Windows host-backed mode)
- Activation: `CLSIDFromProgID` + `CoCreateInstance`
- Invoke: `IDispatch::Invoke`

## Contract Invariants

1. Object token identity is stable for the lifetime of the HAL binding.
2. Native dispatch pointer ownership is bound to COM-state lifecycle, not per-invoke temporary activation.
3. Unsupported or denied paths remain deterministic and must not mutate COM-state.
4. Failure translation is deterministic and tagged by the COM error taxonomy table.
5. Member-name lanes must keep deterministic per-object cache semantics for resolved DISPIDs on native Windows path.
6. Missing third argument in `DispatchInvoke`:
- property-get member lanes may proceed with no argument;
- argument-required member lanes must fail deterministically.

## Deferred Extensions

- Natural VBA member syntax to late-bound dispatch lowering.
- Named/optional argument parity expansion.
- Full generic ProgID/member-name text selector path through current integer-token VM boundary.
