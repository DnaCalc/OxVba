# HAL COM Bridge Scope V1

Status: `design-draft`  
Step: `v190`  
Date: 2026-03-02

## Objective

Define what COM behavior belongs inside HAL, what stays outside, and the minimal implementation scope for v1.

## HAL Boundary

HAL COM trait surface:
- `create_object(prog_id_token) -> object_token`
- `dispatch_invoke(object_token, member_token, arg_token) -> result_token`

This boundary is transport-level and tokenized. Detailed VBA object model semantics remain in compiler/runtime layers.

## Platform Scope

- Windows profiles: COM capability supported.
- Linux/macOS/wasm/null: COM capability unsupported in v1 with deterministic `HAL-E-CAP-UNAVAILABLE`.

## V1 Implementation Scope (Windows)

1. Activation path:
- `CoInitializeEx` lifecycle strategy at host boundary.
- `CLSIDFromProgID` resolution for canonical `CreateObject` paths.
- `CoCreateInstance` activation for in-proc server classes.

2. Invocation path:
- minimal `IDispatch::Invoke` bridging for positional arg calls.
- deterministic failure mapping for unsupported/malformed invocations.

3. Marshaling baseline:
- `VARIANT`, `BSTR`, and scalar numeric/string tokens.
- array/object advanced marshaling tracked as post-v1 extension.

## Non-Goals (V1)

- full Excel object model parity claims,
- event sinks/connection points,
- cross-platform COM emulation,
- custom apartment policy controls beyond basic host runner defaults.

## Clause Candidates

| Clause | Statement | Verification Layer |
|---|---|---|
| `HAL-COM-V1-001` | Non-Windows profiles deterministically reject COM ops with capability-unavailable errors. | HAL conformance |
| `HAL-COM-V1-002` | Windows activation failure surfaces stable HAL code and operation field (`create_object`). | host integration |
| `HAL-COM-V1-003` | Dispatch invoke failures surface stable HAL code and operation field (`dispatch_invoke`). | host integration |
| `HAL-COM-V1-004` | Compile-time mode rejects unsupported COM usage before execution. | host gate tests |

## Design Cycle Deliverables (Follow-on)

- COM bridge adapter design note with ABI and apartment assumptions.
- marshaling matrix tied to Foundation canonical specs.
- empirical conformance probes against real VBA/COM hosts (deferred gate lane).

## Open Items

- explicit apartment threading model in host runner config.
- token<->VARIANT conversion fidelity for arrays/ByRef/ref-object arguments.
- deterministic lifetime and release semantics for object tokens.
