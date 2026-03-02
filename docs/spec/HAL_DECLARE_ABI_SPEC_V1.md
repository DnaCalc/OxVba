# HAL Declare ABI Specification V1

Status: `design-draft`  
Step: `v191`  
Date: 2026-03-02

## Objective

Define a cross-platform `Declare` execution contract for Windows and Linux under HAL governance.

## Scope

VBA-facing features in scope:
- `Declare` function binding by library + symbol/alias,
- calling convention selection where applicable,
- deterministic error contract for binding/invocation failures.

## HAL Placement

`Declare` support is split:
- compile-time: resolver/typechecker validates declaration shape and call-site arity/type subset,
- runtime: HAL dynamic-link domain resolves and invokes symbols.

Current HAL trait anchor:
- `DynamicLinkHal::invoke_symbol(symbol_token, arg_token) -> token`

V1 may introduce richer dynamic-link operations in trait evolution if required by argument packing.

## Platform Model

Windows:
- loader APIs (`LoadLibraryW`, `GetProcAddress`) via host adapter,
- convention support baseline: `stdcall` and `cdecl` (explicitly mapped).

Linux:
- loader APIs (`dlopen`, `dlsym`) via host adapter,
- convention baseline: SysV C ABI (`cdecl` equivalent).

## Type/Marshaling Subset (V1)

Supported first:
- scalar integers and floating point tokens,
- string pointer handles where representation is explicit,
- by-value arguments only (initially).

Deferred:
- complex struct by-value/byref marshaling,
- callback function pointers,
- advanced ownership/lifetime transfer.

## Failure Contract

Binding failure:
- deterministic HAL error code family (`HAL-E-ADAPTER-FAULT` or capability/policy errors as applicable),
- stable operation field and message payload.

Call failure:
- deterministic runtime surfacing through existing VM host-error path.

## Clause Candidates

| Clause | Statement | Verification Layer |
|---|---|---|
| `HAL-DYN-V1-001` | Unsupported profiles reject `Declare` with deterministic capability errors. | HAL conformance |
| `HAL-DYN-V1-002` | Windows/Linux symbol resolution failures produce stable diagnostics. | host integration |
| `HAL-DYN-V1-003` | Calling-convention mismatch failures are deterministic and non-crashing. | host integration |
| `HAL-DYN-V1-004` | Compile-time mode can reject obviously unsupported declaration shapes before runtime. | compiler/host gate |

## Open Items

- exact representation of strings and ownership for boundary crossing.
- whether to expand HAL trait to explicit bind/prepare/invoke stages.
- how to expose per-profile ABI capabilities in descriptor metadata.
