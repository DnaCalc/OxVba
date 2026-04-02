# Browser-Native Wasm Handoff v1

Status: `draft`
Date: 2026-04-03
Scope owner: OxVba web host-shell lane
Canonical path: `docs/spec/BROWSER_NATIVE_WASM_HANDOFF_V1.md`

Related docs:
- `docs/WEB_HOST_SHELL_BASELINE_EVIDENCE.md`
- `docs/spec/WEB_HOST_BRIDGE_CONTRACT_V1.md`
- `docs/worksets/WORKSET_2026-04-02_WEB_WASM_HOST_SHELL_REALIZATION_EXECUTION.md`

---

## 1. Purpose

Define the honest handoff from the delivered desktop-first web host shell baseline to a later browser-native wasm realization lane.

This document exists to keep the current lane narrow:
- the desktop-first shell is real,
- the typed bridge is real,
- the browser-native wasm product lane is not yet delivered.

---

## 2. What Is Already Real

Current landed substrate:
- typed serializable host bridge DTOs in `oxvba-web-host`
- desktop-first shell session orchestration in `oxvba-web-shell`
- workspace load/reload and document overlay flow over direct OxVba APIs
- run/reset flow over direct runtime/session APIs
- diagnostics and output event projection
- bounded Immediate Window command flow against a live runtime session

This means the semantic boundary for a future browser-native host is no longer speculative.

---

## 3. What The Next Browser-Native Lane May Reuse

The later browser-native lane should reuse:
- the `WebHostCommand` / `WebHostEvent` family shape
- the existing DTO projection rules
- the direct OxVba ownership split:
  - `oxvba-project` owns project truth
  - `oxvba-languageservice` owns workspace and semantic query truth
  - `oxvba-host` owns runtime, immediate, and debugger truth

The browser-native lane should not introduce:
- a parallel project model
- a second semantic layer in JavaScript
- ad hoc JSON payloads that drift away from the typed bridge contract

---

## 4. Explicit Non-Claims

The current lane does not yet deliver:
- a `wasm32` target build for the host/session/runtime stack
- browser-side filesystem or workspace persistence policy
- JS callback/host policy realization
- browser worker model or multi-threading strategy
- secure browser remoting/session ownership semantics
- production packaging for web deployment

Those are the true start points for the next lane, not hidden assumptions.

---

## 5. Required Start Points For The Next Lane

The next browser-native wasm work should begin by deciding:

1. Which OxVba crates can compile directly to `wasm32` without host-policy leakage.
2. Whether runtime execution stays in-process in wasm or behind a host/remoting boundary.
3. How project/workspace files are supplied:
   - browser file pickers,
   - IndexedDB,
   - remote workspace source,
   - or a bounded hybrid.
4. How the current typed bridge maps onto the browser transport:
   - direct wasm exports,
   - `wasm-bindgen` shims,
   - worker message channel,
   - or host-backed IPC/remoting.

---

## 6. Exit Condition For This Handoff

This handoff is complete when the repository truth is:
- the desktop-first shell baseline is evidenced,
- the typed bridge contract is reusable,
- and the browser-native lane is unblocked without pretending it has already been realized.
