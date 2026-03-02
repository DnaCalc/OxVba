# HAL Declare Execution Implementation V2

Status: `implemented-subset`
Scope: `v212..v218`
Date: 2026-03-02

## Implemented Pipeline

1. Resolver capture:
- `Declare` statements now record external metadata (`Lib`, `Alias`) in `BoundModule.external_declarations`.

2. Compiler lowering:
- external declare calls lower to `Instruction::IntrinsicInvokeSymbolHost`.
- symbol token is deterministic hash of `(library, alias, declared_name)`.

3. VM execution:
- new instruction is executed through `host_services.dynlink().invoke_symbol(...)`.
- host errors route via existing runtime error mapping and `On Error Resume Next` behavior.

4. Host/HAL gating:
- compile-time preflight now validates dynamic-link policy for declare invocations.
- runtime denial uses stable `HAL-E-POLICY-DENIED [invoke_symbol]` shape.

5. Host-backed subset:
- Windows/Linux host-backed lane resolves known symbols for baseline execution:
  - `host!ping!HostPing` -> `arg + 1`
  - `host!double!HostDouble` -> `arg * 2`
- unresolved symbol token in host-backed lane returns deterministic adapter fault.
6. Lane-A declaration hardening:
- `Declare PtrSafe ...` is required in v1 subset.
- declaration alias is canonicalized deterministically:
  - symbolic alias is normalized to lowercase,
  - ordinal alias must be `#` followed by digits and is normalized (for example `#0007` -> `#7`).
- unsupported declaration shapes are rejected during resolve/compile in current subset:
  - `ByRef` parameters,
  - `Optional` and `ParamArray` parameters,
  - more than one argument.
- supported ABI value surface in current subset:
  - one `ByVal ... As Long` argument,
  - `Function ... As Long` return type.

## Current Scope Boundary

- This is a deterministic subset suitable for language/runtime integration and conformance scaffolding.
- Full native ABI loading (`LoadLibrary`/`dlopen` symbol lookup by textual name and rich marshaling) remains future work.

## Relationship To Formal Contract

The formal contract baseline for next expansion is now captured in:
- [`HAL_DECLARE_ABI_SPEC_V1.md`](HAL_DECLARE_ABI_SPEC_V1.md)
- [`HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md`](HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md)

This implementation currently satisfies only the subset floor (`HAL-DYN-001`) and does not yet implement:
- alias/ordinal normalization conformance checks (`HAL-DYN-002`),
- ptrsafe/declaration-shape policy restrictions (`HAL-DYN-004`, `HAL-DYN-010`),
- Automation/native marshaling legality checks (`HAL-DYN-005..008`),
- expanded dynamic-link failure taxonomy checks (`HAL-DYN-009`).

## Test Evidence

- compiler: declare call lowers to `IntrinsicInvokeSymbolHost`.
- VM: dynlink instruction routes through HAL.
- host: compile-time/runtime deny tests + host-backed success test for known declare symbol.
