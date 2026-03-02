# HAL DoEvents Conformance V1

Status: `working-draft`  
Step: `v189`  
Date: 2026-03-02

## Objective

Define host-visible behavior for `DoEvents` across profiles and host contexts.

## Surface

Trait surface:
- `do_events() -> token`

Current token convention: return `0` on successful pump/yield operation in V1.

## Semantics by Host Context (V1 intent)

1. `windows-gui`:
- process one bounded message-pump cycle in interactive lane,
- must be non-blocking,
- return deterministic token.

2. `windows-headless`:
- no message queue pump requirement,
- may yield scheduler once,
- return deterministic token.

3. `linux-stdio`:
- no GUI queue,
- scheduler yield allowed,
- return deterministic token.

4. `wasm-wasi-local` / `wasm-browser-sandbox`:
- no host GUI queue obligations,
- deterministic no-op/yield semantics.

5. `null-floor`:
- capability unavailable unless explicitly enabled by descriptor; if unavailable, deterministic unsupported error.

## Clause Candidates

| Clause | Statement | Verification Layer |
|---|---|---|
| `HAL-EVT-V1-001` | Supported profiles return success token without blocking. | HAL tests |
| `HAL-EVT-V1-002` | Unsupported profiles return `HAL-E-CAP-UNAVAILABLE`. | HAL conformance |
| `HAL-EVT-V1-003` | Deterministic lanes produce deterministic observable state transitions. | HAL tests |
| `HAL-EVT-V1-004` | Runtime error mapping is stable under policy/unsupported failures. | host integration |

## Conformance Harness Additions

- probe: `events.do_events.basic`
- probe: `events.do_events.unsupported`
- host integration: `On Error Resume Next` capture path for unsupported profile.

## Open Items

- precise fairness guarantees for GUI message pumping are unresolved (`HAL-U-002`).
- whether return token should encode pumped-event count in a future contract version.
