# Web Host Bridge Contract v1

Status: `draft`
Date: 2026-04-03
Scope owner: OxVBA web host-shell lane
Canonical path: `docs/spec/WEB_HOST_BRIDGE_CONTRACT_V1.md`

Related docs:
- `docs/worksets/WORKSET_2026-04-02_WEB_WASM_HOST_SHELL_REALIZATION_EXECUTION.md`
- `docs/LANGUAGE_SERVICE_PUBLIC_INTERFACE.md`
- `docs/spec/OXVBA_IMMEDIATE_EVALUATOR_CONTRACT_V1.md`
- `docs/spec/OXVBA_DEBUGGER_CONTRACT_V1.md`

---

## 1. Purpose

Define the first typed, serializable bridge contract for the desktop-first web host-shell lane.

This contract is the boundary between:
- the direct OxVba Rust APIs
- a desktop host process
- and a web UI frontend

It is intentionally not a second semantic layer.

---

## 2. Governing Rules

1. The bridge must sit over existing direct OxVba APIs.
2. Workspace/project truth remains in `oxvba-project` and `oxvba-languageservice`.
3. Immediate and debugger surfaces remain owned by `oxvba-host`.
4. The bridge contract is transport-neutral and serializable.
5. Browser-native wasm is not required for this contract.

---

## 3. First Contract Surface

The v1 contract must cover these command families:

- workspace/session
  - load workspace
  - reload workspace
  - list documents
  - set document text
  - close document
- execution/session lifecycle
  - run project
  - reset runtime
- immediate
  - evaluate immediate command/query/expression
- debugger
  - start
  - continue
  - step into / over / out
  - evaluate in paused context

The v1 event/result surface must cover:

- workspace loaded
- diagnostics updated
- output line
- run-state changes
- immediate result
- debug pause state
- error

---

## 4. Projection Rule

The bridge must project direct OxVba types into web-friendly DTOs rather than leaking internal structs directly into shell/frontend code.

Examples:
- `HostWorkspaceDocument` -> web document summary
- `SpannedDiagnostic` -> web diagnostic
- `ImmediateEvaluationResult` -> web immediate result
- `DebugPauseState` -> web debug pause state

These projections must preserve semantic meaning while simplifying frontend consumption.

---

## 5. Non-Goals

This contract does not by itself deliver:
- the actual desktop shell container
- a browser-native wasm runtime
- full IDE UX
- network remoting or multi-client session semantics

Those remain later delivery lanes.
