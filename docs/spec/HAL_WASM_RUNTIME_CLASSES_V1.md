# HAL WASM Runtime Classes V1

Status: `working-draft`  
Step: `v193`  
Date: 2026-03-02

## Objective

Define wasm runtime subtypes for:
- local/sandbox utility use,
- browser execution,
and map each to explicit HAL capability guarantees.

## Runtime Classes

1. `wasi-local`
- Intended for local CLI/sandboxed host contexts.
- Deterministic core capabilities with virtualization-enabled interaction policy.

2. `browser-sandbox`
- Intended for in-browser execution with strict host isolation.
- No UI capability in current baseline; no process/FS/COM/dynlink.

## Capability Matrix

| Runtime class | UI | EventPump | FileSystemIo | ProcessEnv | COM | TimeLocale | DynamicLinking | Diagnostics |
|---|---|---|---|---|---|---|---|---|
| `wasi-local` | supported (virtualized only) | supported | unsupported | unsupported | unsupported | supported | unsupported | supported |
| `browser-sandbox` | unsupported | supported | unsupported | unsupported | unsupported | supported | unsupported | supported |

## Policy Interaction

- Deterministic presets remain deterministic by contract.
- `interactive-dev` on wasm does not imply host-backed native OS integration.
- UI in `wasi-local` still follows virtualization policy.

## Conformance Requirements

Harness must execute both wasm runtime classes for each lane:
1. runtime
2. compile-time
3. interactive-dev

Required evidence fields:
- runtime class,
- profile/lane,
- clause pass/fail,
- governance notices.

## Clause Candidates

| Clause | Statement | Verification Layer |
|---|---|---|
| `HAL-WASM-V1-001` | `wasi-local` and `browser-sandbox` descriptors expose distinct capability sets. | HAL conformance |
| `HAL-WASM-V1-002` | Unsupported domains return deterministic capability-unavailable errors. | HAL conformance |
| `HAL-WASM-V1-003` | Time/diag/event domains remain available with deterministic token semantics. | HAL tests |
| `HAL-WASM-V1-004` | UI behavior in `wasi-local` is virtualization-governed, not native prompt. | HAL tests |

## Open Items

- whether to introduce a third wasm class for embedded host bridges.
- whether sandboxed virtual FS should be added in future for deterministic fixture loading.
